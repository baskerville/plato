EX=demo
DEVICE_IP ?= "10.11.99.1"

ssh root@$(DEVICE_IP) 'kill -9 `pidof $(EX)` || true; systemctl stop xochitl || true'
scp ./target/armv7-unknown-linux-gnueabihf/release/examples/$(EX) root@$(DEVICE_IP):~/
ssh root@$(DEVICE_IP) './$(EX)' && $(MAKE) start-xochitl
