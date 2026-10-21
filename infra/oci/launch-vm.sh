#!/usr/bin/env bash
# VM A1 (2 OCPU / 12 GB / 50 GB, dentro do Always Free: teto 4 OCPU, 24 GB, 200 GB) + IP público reservado.
# Uso: COMPARTMENT_ID=... SUBNET_ID=... IMAGE_ID=... SSH_KEY_FILE=~/.ssh/id_ed25519.pub ./launch-vm.sh
# Repete a cada 60 s enquanto o OCI responder "Out of host capacity".
set -uo pipefail
: "${COMPARTMENT_ID:?}" "${SUBNET_ID:?}" "${IMAGE_ID:?}" "${SSH_KEY_FILE:?}"
AD=$(oci iam availability-domain list -c "$COMPARTMENT_ID" --query 'data[0].name' --raw-output)

until INSTANCE=$(oci compute instance launch -c "$COMPARTMENT_ID" --availability-domain "$AD" \
    --shape VM.Standard.A1.Flex --shape-config '{"ocpus":2,"memoryInGBs":12}' \
    --image-id "$IMAGE_ID" --boot-volume-size-in-gbs 50 --subnet-id "$SUBNET_ID" \
    --assign-public-ip false --display-name hermes --ssh-authorized-keys-file "$SSH_KEY_FILE" \
    --freeform-tags '{"project":"hermes"}' --wait-for-state RUNNING --query 'data.id' --raw-output 2>/tmp/launch.err); do
  grep -q "Out of host capacity" /tmp/launch.err || { cat /tmp/launch.err; exit 1; }
  echo "sem capacidade, tentando de novo em 60 s"; sleep 60
done

VNIC=$(oci compute instance list-vnics --instance-id "$INSTANCE" --query 'data[0].id' --raw-output)
PRIV=$(oci network private-ip list --vnic-id "$VNIC" --query 'data[0].id' --raw-output)
oci network public-ip create -c "$COMPARTMENT_ID" --lifetime RESERVED --private-ip-id "$PRIV" \
  --display-name hermes-ip --freeform-tags '{"project":"hermes"}' --query 'data."ip-address"' --raw-output | sed 's/^/PUBLIC_IP=/'
echo "INSTANCE_ID=$INSTANCE"
