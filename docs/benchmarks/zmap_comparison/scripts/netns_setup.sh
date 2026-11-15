#!/bin/sh
# Two network namespaces joined by one veth pair, inside a container started
# with --network none. Nothing here has a route to any real network.
#   scan: veth0 192.0.2.1/24 (TEST-NET-1), no default route
#   resp: veth1 192.0.2.2/24, AnyIP local route for 198.18.0.0/15 (RFC 2544
#         benchmarking range) so its kernel answers every probed address:
#         SYN-ACK on 80 and 8080 (listener), RST on 443 and 8443 (closed).
set -eu
# Refuse unless the container's own namespace has nothing but loopback.
links=$(ip -o link show | awk -F': ' '{print $2}' | sed 's/@.*//' | grep -v '^lo$' || true)
if [ -n "$links" ]; then echo "REFUSE: non-loopback interface in root ns: $links" >&2; exit 1; fi
if ip route show | grep -q .; then echo "REFUSE: root ns has routes" >&2; ip route show >&2; exit 1; fi
ip netns add scan
ip netns add resp
ip link add veth0 netns scan type veth peer name veth1 netns resp
ip -n scan link set lo up
ip -n resp link set lo up
ip -n scan addr add 192.0.2.1/24 dev veth0
ip -n resp addr add 192.0.2.2/24 dev veth1
ip -n scan link set veth0 up
ip -n resp link set veth1 up
ip -n resp route add local 198.18.0.0/15 dev lo
# Static neighbours: replies come from AnyIP sources outside 192.0.2.0/24,
# so dynamic ARP from the responder does not resolve.
MAC0=$(ip -n scan link show veth0 | awk '/link\/ether/ {print $2}')
MAC1=$(ip -n resp link show veth1 | awk '/link\/ether/ {print $2}')
ip -n resp neigh add 192.0.2.1 lladdr "$MAC0" dev veth1 nud permanent
ip -n scan neigh add 192.0.2.2 lladdr "$MAC1" dev veth0 nud permanent
ip netns exec scan sysctl -qw net.ipv4.conf.all.rp_filter=0
ip netns exec scan sysctl -qw net.ipv4.conf.veth0.rp_filter=0
ip netns exec resp sysctl -qw net.ipv4.conf.all.rp_filter=0
ip netns exec resp sysctl -qw net.ipv4.conf.veth1.rp_filter=0
ip netns exec resp sysctl -qw net.ipv4.tcp_syncookies=2
ip netns exec resp sysctl -qw net.core.somaxconn=65535
# Assert: the scan namespace can reach only 192.0.2.0/24.
if ip -n scan route show | grep -v '^192\.0\.2\.0/24 dev veth0' | grep -q .; then
  echo "REFUSE: unexpected route in scan ns" >&2; ip -n scan route show >&2; exit 1
fi
ip netns exec resp python3 -c '
import socket, time, os
ss = []
for p in (80, 8080):
    s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(("0.0.0.0", p)); s.listen(65535); ss.append(s)
open("/tmp/listener.ready", "w").close()
while True: time.sleep(3600)
' &
while [ ! -e /tmp/listener.ready ]; do sleep 0.1; done
ip -n resp link show veth1 | awk '/link\/ether/ {print $2}' > /tmp/gwmac
echo "netns ready, gateway mac $(cat /tmp/gwmac)"
