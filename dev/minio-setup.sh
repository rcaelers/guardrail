#!/bin/sh
set -ex

echo "Starting MinIO setup script..."

# Wait for MinIO to be fully up
echo "Waiting for MinIO to be fully up..."
sleep 5

# Configure MinIO client
echo "Configuring MinIO client..."
mc alias set myminio http://minio:9000 admin minioadmin
echo "MinIO client configured successfully."

# Create guardrail bucket if it doesn't exist
echo "Creating guardrail bucket..."
mc mb --ignore-existing myminio/guardrail
echo "Bucket creation completed."

# Create guardrail user if it doesn't exist
echo "Creating guardrail user..."
# Check if user exists first
if ! mc admin user info myminio guardrail >/dev/null 2>&1; then
  mc admin user add myminio guardrail guardrail
  echo "User created."
else
  echo "User already exists."
fi

# Create policy file
echo "Creating policy file..."
cat >/tmp/policy.json <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:*"],
      "Resource": ["arn:aws:s3:::guardrail", "arn:aws:s3:::guardrail/*"]
    }
  ]
}
EOF
echo "Policy file created."

# Create policy
echo "Creating policy..."
# Try to create policy, but don't fail if it already exists
if ! mc admin policy info myminio guardrail-readwrite >/dev/null 2>&1; then
  mc admin policy create myminio guardrail-readwrite /tmp/policy.json
  echo "Policy created."
else
  echo "Policy already exists."
fi

# Attach policy to user
echo "Attaching policy to user..."
mc admin policy attach myminio guardrail-readwrite --user guardrail
echo "Policy attached successfully."

echo "MinIO setup completed successfully!"
