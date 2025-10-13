sudo nmcli connection modify ovsbr0 connection.autoconnect yes && \
sudo nmcli connection modify ovsbr0if connection.autoconnect yes && \
sudo nmcli connection modify ovsbr0portint connection.autoconnect yes && \
sudo nmcli connection modify ovsbr0uplinkens1 connection.autoconnect yes && \
sudo nmcli connection modify ovsbr1 connection.autoconnect yes && \
sudo nmcli connection modify ovsbr1if connection.autoconnect yes && \
sudo nmcli connection modify ovsbr1portint connection.autoconnect yes
