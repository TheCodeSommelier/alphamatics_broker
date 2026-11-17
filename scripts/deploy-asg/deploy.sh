#!/usr/bin/env bash
set -euo pipefail

# Deploy one pinned ARM64 broker image. NATS stays on its own host.
#
# Flow: validate inputs and AWS resources -> create a launch template version ->
# refresh the ASG -> wait for healthy NLB targets.

fail() {
  echo "::error::$*" >&2
  exit 1
}

log() {
  echo "::notice::$*"
}

# Required release settings
: "${DEPLOY_STAGE:?Set DEPLOY_STAGE to staging or production}"
: "${ASG_NAME:?Set ASG_NAME}"
: "${AWS_REGION:?Set AWS_REGION}"
: "${AWS_SECRET_ID:?Set AWS_SECRET_ID}"
: "${BROKER_IMAGE:?Set BROKER_IMAGE to an ECR digest reference}"
: "${NATS_TELEMATICS_MAX_BYTES:?Set NATS_TELEMATICS_MAX_BYTES}"
: "${NATS_COMMANDS_MAX_BYTES:?Set NATS_COMMANDS_MAX_BYTES}"
NATS_MAX_AGE_HOURS="${NATS_MAX_AGE_HOURS:-72}"

case "$DEPLOY_STAGE" in
  staging | production) ;;
  *) fail "Invalid stage: $DEPLOY_STAGE" ;;
esac

if [[ ! "$AWS_REGION" =~ ^[a-z0-9-]+$ ]] ||
   [[ ! "$AWS_SECRET_ID" =~ ^[a-zA-Z0-9/_+=.@:-]+$ ]]; then
  fail 'Invalid AWS region or secret ID'
fi

for value in "$NATS_MAX_AGE_HOURS" "$NATS_TELEMATICS_MAX_BYTES" "$NATS_COMMANDS_MAX_BYTES"; do
  [[ "$value" =~ ^[0-9]+$ && "$value" != 0 ]] ||
    fail 'NATS limits must be positive integers'
done

for command in aws jq base64; do
  command -v "$command" >/dev/null || fail "Missing $command"
done

[[ -f compose.yaml && -f scripts/user-data-launch-template.sh ]] ||
  fail 'Missing broker deployment files'

export AWS_REGION AWS_PAGER=""

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

# Verify that the immutable image exists in this AWS Region.
[[ "$BROKER_IMAGE" =~ ^([0-9]{12}\.dkr\.ecr\.([a-z0-9-]+)\.amazonaws\.com)/([a-zA-Z0-9._/-]+)@(sha256:[0-9a-f]{64})$ ]] ||
  fail 'BROKER_IMAGE must be an ECR digest reference'

registry="${BASH_REMATCH[1]}"
image_region="${BASH_REMATCH[2]}"
image_repo="${BASH_REMATCH[3]}"
image_digest="${BASH_REMATCH[4]}"
[[ "$image_region" == "$AWS_REGION" ]] ||
  fail 'Image and deployment regions differ'

aws ecr describe-images \
  --repository-name "$image_repo" \
  --image-ids "imageDigest=$image_digest" \
  --query 'imageDetails[0].imageDigest' \
  --output text |
  grep -Fx "$image_digest" >/dev/null ||
  fail 'Broker image digest is missing from ECR'

# Read the ASG and ensure this deployment cannot accidentally use production
# resources when DEPLOY_STAGE=staging.
asg="$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "$ASG_NAME" \
  --output json | jq -c '.AutoScalingGroups[0] // empty')"
[[ -n "$asg" ]] || fail "ASG not found: $ASG_NAME"

lt_id="$(jq -r '.LaunchTemplate.LaunchTemplateId // empty' <<< "$asg")"
lt_source="$(jq -r '.LaunchTemplate.Version // empty' <<< "$asg")"
[[ -n "$lt_id" && -n "$lt_source" ]] || fail 'ASG must use a direct launch template'

target_group="$(jq -r '.TargetGroupARNs[0] // empty' <<< "$asg")"
[[ -n "$target_group" && "$(jq -r '.TargetGroupARNs | length' <<< "$asg")" -eq 1 ]] ||
  fail 'ASG must have exactly one target group'

desired="$(jq -r '.DesiredCapacity' <<< "$asg")"
[[ "$desired" -ge 1 ]] || fail 'ASG desired capacity must be at least one'
[[ "$(jq -r '.Instances | length' <<< "$asg")" -eq "$desired" ]] ||
  fail 'Wait for ASG capacity to settle before deploying'
jq -e '.Instances | all(.LifecycleState == "InService")' <<< "$asg" >/dev/null ||
  fail 'Wait for all ASG instances to enter service'

if [[ "$DEPLOY_STAGE" == staging ]]; then
  prod_name="${PROD_ASG_NAME:-alphamatics-prod-broker-asg}"
  prod="$(aws autoscaling describe-auto-scaling-groups \
    --auto-scaling-group-names "$prod_name" \
    --output json | jq -c '.AutoScalingGroups[0] // empty')"

  if [[ -n "$prod" ]]; then
    [[ "$ASG_NAME" != "$prod_name" ]] || fail 'Staging ASG is the production ASG'
    [[ "$lt_id" != "$(jq -r '.LaunchTemplate.LaunchTemplateId // empty' <<< "$prod")" ]] || fail 'Staging shares the production launch template'
    [[ "$target_group" != "$(jq -r '.TargetGroupARNs[0] // empty' <<< "$prod")" ]] || fail 'Staging shares the production target group'
  fi
fi

refreshes="$(aws autoscaling describe-instance-refreshes \
  --auto-scaling-group-name "$ASG_NAME" \
  --max-records 1 \
  --output json)"
jq -e '
  [.InstanceRefreshes[] |
    select(.Status == "Pending" or
           .Status == "InProgress" or
           .Status == "Cancelling" or
           .Status == "RollbackInProgress")
  ] | length == 0
' <<< "$refreshes" >/dev/null || fail 'An instance refresh is already running'

# Check the launch template, target group, load balancer, and security groups.
lt="$(aws ec2 describe-launch-template-versions \
  --launch-template-id "$lt_id" \
  --versions "$lt_source" \
  --output json | jq -c '.LaunchTemplateVersions[0]')"

lt_number="$(jq -r '.VersionNumber' <<< "$lt")"
lt_type="$(jq -r '.LaunchTemplateData.InstanceType // empty' <<< "$lt")"
lt_ami="$(jq -r '.LaunchTemplateData.ImageId // empty' <<< "$lt")"
[[ -n "$lt_type" && -n "$lt_ami" ]] || fail 'Launch template needs an instance type and AMI'
[[ "$(jq -r '.LaunchTemplateData.MetadataOptions.HttpPutResponseHopLimit // 1' <<< "$lt")" -ge 2 ]] ||
  fail 'IMDS hop limit must be at least two for Docker'
[[ "$(jq -r '.LaunchTemplateData.IamInstanceProfile.Arn // .LaunchTemplateData.IamInstanceProfile.Name // empty' <<< "$lt")" != '' ]] ||
  fail 'Launch template needs an instance profile for ECR and Secrets Manager'

aws ec2 describe-instance-types --instance-types "$lt_type" --output json |
  jq -e '.InstanceTypes[0].ProcessorInfo.SupportedArchitectures | index("arm64") != null' >/dev/null ||
  fail 'Instance type is not ARM64'
aws ec2 describe-images --image-ids "$lt_ami" --output json |
  jq -e '.Images[0].Architecture == "arm64"' >/dev/null ||
  fail 'AMI is not ARM64'

mapfile -t instance_sgs < <(
  jq -r '
    .LaunchTemplateData.SecurityGroupIds[]?,
    .LaunchTemplateData.NetworkInterfaces[]?.Groups[]?
  ' <<< "$lt" | sort -u
)
[[ "${#instance_sgs[@]}" -gt 0 ]] ||
  fail 'Launch template needs an explicit broker security group'

tg="$(aws elbv2 describe-target-groups \
  --target-group-arns "$target_group" \
  --output json | jq -c '.TargetGroups[0]')"
[[ "$(jq -r '.TargetType' <<< "$tg")" == instance &&
   "$(jq -r '.Protocol' <<< "$tg")" == TCP &&
   "$(jq -r '.Port' <<< "$tg")" == 4001 ]] ||
  fail 'Target group must use instance targets and TCP:4001'
[[ "$(jq -r '.HealthCheckProtocol' <<< "$tg")" == TCP ]] ||
  fail 'Target group must use TCP health checks'

lb_arn="$(jq -r '.LoadBalancerArns[0] // empty' <<< "$tg")"
[[ -n "$lb_arn" ]] || fail 'Target group has no load balancer'
lb="$(aws elbv2 describe-load-balancers \
  --load-balancer-arns "$lb_arn" \
  --output json | jq -c '.LoadBalancers[0]')"
[[ "$(jq -r '.Type' <<< "$lb")" == network ]] ||
  fail 'Broker target group must use a Network Load Balancer'

aws elbv2 describe-listeners --load-balancer-arn "$lb_arn" --output json |
  jq -e --arg tg "$target_group" '
    [.Listeners[] |
      select(.Protocol == "TCP" and .Port == 4001) |
      .DefaultActions[]? |
      select(.TargetGroupArn == $tg)
    ] | length > 0
  ' >/dev/null ||
  fail 'NLB has no TCP:4001 listener forwarding to the target group'

lb_sg="$(jq -r '.SecurityGroups[0] // empty' <<< "$lb")"
[[ -n "$lb_sg" ]] || fail 'NLB needs a security group'

for sg in "${instance_sgs[@]}"; do
  permissions="$(aws ec2 describe-security-groups \
    --group-ids "$sg" \
    --output json | jq -c '.SecurityGroups[0].IpPermissions')"
  jq -e --arg source "$lb_sg" '
    [.[] |
      select(.IpProtocol == "tcp" and .FromPort <= 4001 and .ToPort >= 4001) |
      .UserIdGroupPairs[]? |
      select(.GroupId == $source)
    ] | length > 0
  ' <<< "$permissions" >/dev/null ||
    fail "Instance SG $sg does not allow TCP:4001 from NLB SG $lb_sg"
done

# Combine the host bootstrap, broker Compose file, and pinned image into EC2
# user data. Only this broker ASG receives the new launch template version.
compose64="$(base64 < compose.yaml | tr -d '\n')"
user_data="$workdir/user-data.sh"
cat scripts/user-data-launch-template.sh > "$user_data"
cat >> "$user_data" <<EOF

# ALPHAMATICS_BROKER_RELEASE $BROKER_IMAGE
mkdir -p /opt/broker
echo '$compose64' | base64 -d > /opt/broker/compose.yaml
cat > /opt/broker/.image.env <<'IMAGE_ENV'
BROKER_IMAGE=$BROKER_IMAGE
AWS_SECRET_ID=$AWS_SECRET_ID
AWS_REGION=$AWS_REGION
ENV=$DEPLOY_STAGE
NATS_MAX_AGE_HOURS=$NATS_MAX_AGE_HOURS
NATS_TELEMATICS_MAX_BYTES=$NATS_TELEMATICS_MAX_BYTES
NATS_COMMANDS_MAX_BYTES=$NATS_COMMANDS_MAX_BYTES
IMAGE_ENV
chmod 600 /opt/broker/.image.env
aws ecr get-login-password --region '$AWS_REGION' | docker login --username AWS --password-stdin '$registry'
cd /opt/broker
docker compose --env-file .image.env pull broker
docker compose --env-file .image.env up -d --no-build broker
for ((attempt=0; attempt<60; attempt++)); do
  if (echo > /dev/tcp/127.0.0.1/4001) 2>/dev/null; then
    echo 'Broker TCP listener is ready'
    exit 0
  fi
  sleep 5
done
echo 'Broker did not start listening on TCP:4001' >&2
exit 1
EOF
[[ "$(wc -c < "$user_data")" -le 16384 ]] ||
  fail 'EC2 user data exceeds 16 KiB'

userdata64="$(base64 < "$user_data" | tr -d '\n')"
lt_data="$(jq -nc --arg data "$userdata64" '{UserData:$data}')"
new_version="$(aws ec2 create-launch-template-version \
  --launch-template-id "$lt_id" \
  --source-version "$lt_number" \
  --version-description "AlphaMatics broker $DEPLOY_STAGE ${RELEASE_SHA:-${GITHUB_SHA:-manual}}" \
  --launch-template-data "$lt_data" \
  --query 'LaunchTemplateVersion.VersionNumber' \
  --output text)"
[[ "$new_version" =~ ^[0-9]+$ ]] || fail 'Could not create launch template version'
log "Created launch template $lt_id version $new_version"

# Keep the previous instance serving while the replacement starts.
max="$(jq -r '.MaxSize' <<< "$asg")"
if (( max < desired * 2 )); then
  aws autoscaling update-auto-scaling-group \
    --auto-scaling-group-name "$ASG_NAME" \
    --max-size "$((desired * 2))"
fi

previous_user_data="$(jq -r '.LaunchTemplateData.UserData // empty' <<< "$lt" | base64 -d)"
rollback=false
if [[ "$lt_source" =~ ^[0-9]+$ &&
      "$previous_user_data" == *ALPHAMATICS_BROKER_RELEASE* ]]; then
  rollback=true
fi

desired_config="$(jq -nc \
  --arg id "$lt_id" \
  --arg version "$new_version" \
  '{LaunchTemplate:{LaunchTemplateId:$id,Version:$version}}')"
preferences="$(jq -nc \
  --argjson rollback "$rollback" \
  '{MinHealthyPercentage:100,MaxHealthyPercentage:200,InstanceWarmup:120,SkipMatching:false,AutoRollback:$rollback}')"
refresh_id="$(aws autoscaling start-instance-refresh \
  --auto-scaling-group-name "$ASG_NAME" \
  --desired-configuration "$desired_config" \
  --preferences "$preferences" \
  --query InstanceRefreshId \
  --output text)"
log "Started instance refresh $refresh_id (automatic rollback: $rollback)"

# A successful refresh means the new instances were accepted by ASG. Then
# explicitly verify that every replacement is healthy in the NLB target group.
for ((attempt=0; attempt<225; attempt++)); do
  refresh="$(aws autoscaling describe-instance-refreshes \
    --auto-scaling-group-name "$ASG_NAME" \
    --instance-refresh-ids "$refresh_id" \
    --output json | jq -c '.InstanceRefreshes[0]')"
  status="$(jq -r '.Status' <<< "$refresh")"

  case "$status" in
    Successful) break ;;
    Failed|Cancelled|RollbackSuccessful|RollbackFailed)
      fail "Instance refresh $status: $(jq -r '.StatusReason // "no reason returned"' <<< "$refresh")"
      ;;
  esac

  sleep 20
done
[[ "$status" == Successful ]] ||
  fail "Instance refresh $refresh_id did not finish within 75 minutes"

for ((attempt=0; attempt<60; attempt++)); do
  current_asg="$(aws autoscaling describe-auto-scaling-groups \
    --auto-scaling-group-names "$ASG_NAME" \
    --output json | jq -c '.AutoScalingGroups[0]')"
  mapfile -t current_instances < <(
    jq -r --arg version "$new_version" '
      .Instances[] |
      select(.LifecycleState == "InService" and .LaunchTemplate.Version == $version) |
      .InstanceId
    ' <<< "$current_asg"
  )

  if [[ "${#current_instances[@]}" -eq "$desired" ]]; then
    ids="$(printf '%s\n' "${current_instances[@]}" | jq -R . | jq -s .)"
    health="$(aws elbv2 describe-target-health \
      --target-group-arn "$target_group" \
      --output json)"

    if jq -e --argjson ids "$ids" '
      . as $health |
      all($ids[];
        . as $id |
        [$health.TargetHealthDescriptions[] |
          select(.Target.Id == $id and .TargetHealth.State == "healthy")
        ] | length > 0
      )
    ' <<< "$health" >/dev/null; then
      aws autoscaling update-auto-scaling-group \
        --auto-scaling-group-name "$ASG_NAME" \
        --health-check-type ELB \
        --health-check-grace-period 600
      log "Broker target healthy; instance refresh $refresh_id completed"
      exit 0
    fi
  fi

  sleep 10
done

fail 'Refreshed broker instances did not become healthy in the NLB target group'
