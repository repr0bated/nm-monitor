sudo nmcli connection add type ovs-bridge conn.interface ovsbr0 con-name ovsbr0 connection.id ovsbr0 connection.autoconnect no ipv4.method auto ipv6.method auto && \
sudo nmcli connection add type ovs-port conn.interface ovsbr0portint master ovsbr0 con-name ovsbr0portint connection.id ovsbr0portint connection.autoconnect no && \
sudo nmcli connection add type ovs-interface slave-type ovs-port conn.interface ovsbr0if master ovsbr0portint con-name ovsbr0if connection.id ovsbr0if connection.autoconnect no ipv4.method auto ipv6.method auto && \
sudo nmcli connection add type ovs-port conn.interface ovsbr0portens1 master ovsbr0 con-name ovsbr0portens1 connection.id ovsbr0portens1 connection.autoconnect no && \
sudo nmcli connection add type ethernet conn.interface ens1 master ovsbr0portens1 con-name ovsbr0uplinkens1 connection.id ovsbr0uplinkens1 connection.autoconnect no && \
sudo nmcli connection add type ovs-bridge conn.interface ovsbr1 con-name ovsbr1 connection.id ovsbr1 connection.autoconnect no ipv4.method auto ipv6.method auto && \
sudo nmcli connection add type ovs-port conn.interface ovsbr1portint master ovsbr1 con-name ovsbr1portint connection.id ovsbr1portint connection.autoconnect no && \
sudo nmcli connection add type ovs-interface slave-type ovs-port conn.interface ovsbr1if master ovsbr1portint con-name ovsbr1if connection.id ovsbr1if connection.autoconnect no ipv4.method auto ipv6.method auto
