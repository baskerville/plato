EX=plato
DEVICE_IP="10.11.99.1"

ssh root@$DEVICE_IP 'kill -9 plato || true; systemctl stop xochitl || true'
scp -r ./dist root@$DEVICE_IP:~/
ssh root@$DEVICE_IP 'LD_LIBRARY_PATH=./libs ./plato' 