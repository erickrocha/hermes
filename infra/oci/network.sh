#!/usr/bin/env bash
# Rede do Hermes no OCI (tudo Always Free): VCN + sub-rede pública + IGW.
# Uso: COMPARTMENT_ID=ocid1... SSH_CIDR=1.2.3.4/32 ./network.sh
# Não é idempotente: roda uma vez; para refazer, apague a VCN antes.
set -euo pipefail
: "${COMPARTMENT_ID:?}" "${SSH_CIDR:?}"
TAGS='{"project":"hermes"}'
o() { oci "$@" --query 'data.id' --raw-output; }

VCN=$(o network vcn create -c "$COMPARTMENT_ID" --cidr-block 10.0.0.0/16 --display-name hermes-vcn --dns-label hermes --freeform-tags "$TAGS" --wait-for-state AVAILABLE)
IGW=$(o network internet-gateway create -c "$COMPARTMENT_ID" --vcn-id "$VCN" --is-enabled true --display-name hermes-igw --freeform-tags "$TAGS" --wait-for-state AVAILABLE)

RT=$(oci network vcn get --vcn-id "$VCN" --query 'data."default-route-table-id"' --raw-output)
oci network route-table update --rt-id "$RT" --force \
  --route-rules "[{\"destination\":\"0.0.0.0/0\",\"destinationType\":\"CIDR_BLOCK\",\"networkEntityId\":\"$IGW\"}]" >/dev/null

# SSH só do seu IP; 80/443 abertos (Traefik); ICMP 3,4 exigido pelo OCI para path MTU.
tcp() { echo "{\"protocol\":\"6\",\"source\":\"$1\",\"tcpOptions\":{\"destinationPortRange\":{\"min\":$2,\"max\":$2}}}"; }
SL=$(oci network vcn get --vcn-id "$VCN" --query 'data."default-security-list-id"' --raw-output)
oci network security-list update --security-list-id "$SL" --force \
  --ingress-security-rules "[$(tcp "$SSH_CIDR" 22),$(tcp 0.0.0.0/0 80),$(tcp 0.0.0.0/0 443),{\"protocol\":\"1\",\"source\":\"0.0.0.0/0\",\"icmpOptions\":{\"type\":3,\"code\":4}}]" \
  --egress-security-rules '[{"protocol":"all","destination":"0.0.0.0/0"}]' >/dev/null

SUBNET=$(o network subnet create -c "$COMPARTMENT_ID" --vcn-id "$VCN" --cidr-block 10.0.1.0/24 --display-name hermes-public --dns-label pub --freeform-tags "$TAGS" --wait-for-state AVAILABLE)
echo "VCN_ID=$VCN"; echo "SUBNET_ID=$SUBNET"
