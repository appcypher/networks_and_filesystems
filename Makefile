.PHONY: build install uninstall install_tun uninstall_tun clean

INSTALL_PATH=/usr/local/bin
SYSTEMD_PATH=/etc/systemd/system
LAUNCHD_PATH=/Library/LaunchDaemons

build:
	sudo cargo build --release -p network

install: build
	# Install subnet daemon binary
	sudo install -m 755 target/release/subnet_daemon $(INSTALL_PATH)/subnet-daemon
	# Create log files with proper permissions
	sudo touch /var/log/subnet_daemon.log /var/log/subnet_daemon.err
	sudo chmod 644 /var/log/subnet_daemon.log /var/log/subnet_daemon.err
	sudo chown root:wheel /var/log/subnet_daemon.log /var/log/subnet_daemon.err
	# Install service files based on OS
	@if [ "$$(uname)" = "Darwin" ]; then \
		sudo install -m 644 network/com.subnet.daemon.plist $(LAUNCHD_PATH)/com.subnet.daemon.plist; \
		sudo launchctl load $(LAUNCHD_PATH)/com.subnet.daemon.plist; \
	elif [ "$$(uname)" = "Linux" ]; then \
		sudo install -m 644 network/subnet_daemon.service $(SYSTEMD_PATH)/subnet_daemon.service; \
		sudo systemctl daemon-reload; \
		sudo systemctl enable subnet_daemon.service; \
		sudo systemctl start subnet_daemon.service; \
	fi

uninstall:
	# Stop and remove service based on OS
	@if [ "$$(uname)" = "Darwin" ]; then \
		sudo launchctl unload $(LAUNCHD_PATH)/com.subnet.daemon.plist || true; \
		sudo rm -f $(LAUNCHD_PATH)/com.subnet.daemon.plist; \
	elif [ "$$(uname)" = "Linux" ]; then \
		sudo systemctl stop subnet_daemon.service || true; \
		sudo systemctl disable subnet_daemon.service || true; \
		sudo rm -f $(SYSTEMD_PATH)/subnet_daemon.service; \
		sudo systemctl daemon-reload; \
	fi
	# Remove binary and logs
	sudo rm -f $(INSTALL_PATH)/subnet-daemon
	sudo rm -f /var/log/subnet_daemon.log /var/log/subnet_daemon.err
	sudo rm -f /var/run/subnet_daemon.pid

install_tun: build
	# Install TUN daemon binary
	sudo install -m 755 target/release/tun_daemon $(INSTALL_PATH)/tun-daemon
	# Create log files with proper permissions
	sudo touch /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chmod 644 /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chown root:wheel /var/log/tun_daemon.log /var/log/tun_daemon.err
	# Install service files based on OS
	@if [ "$$(uname)" = "Darwin" ]; then \
		sudo install -m 644 network/com.tun.daemon.plist $(LAUNCHD_PATH)/com.tun.daemon.plist; \
		sudo launchctl load $(LAUNCHD_PATH)/com.tun.daemon.plist; \
	elif [ "$$(uname)" = "Linux" ]; then \
		sudo install -m 644 network/tun_daemon.service $(SYSTEMD_PATH)/tun_daemon.service; \
		sudo systemctl daemon-reload; \
		sudo systemctl enable tun_daemon.service; \
		sudo systemctl start tun_daemon.service; \
	fi

uninstall_tun:
	# Stop and remove service based on OS
	@if [ "$$(uname)" = "Darwin" ]; then \
		sudo launchctl unload $(LAUNCHD_PATH)/com.tun.daemon.plist || true; \
		sudo rm -f $(LAUNCHD_PATH)/com.tun.daemon.plist; \
	elif [ "$$(uname)" = "Linux" ]; then \
		sudo systemctl stop tun_daemon.service || true; \
		sudo systemctl disable tun_daemon.service || true; \
		sudo rm -f $(SYSTEMD_PATH)/tun_daemon.service; \
		sudo systemctl daemon-reload; \
	fi
	# Remove binary and logs
	sudo rm -f $(INSTALL_PATH)/tun-daemon
	sudo rm -f /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo rm -f /var/run/tun_daemon.pid

clean:
	cargo clean
