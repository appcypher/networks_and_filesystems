.PHONY: build install uninstall clean

UNAME := $(shell uname)
NETWORK_DIR := network
RELEASE_BIN := target/release/tun_daemon

build:
	sudo cargo build --release --bin tun_daemon

install: build
	@if [ ! -f $(RELEASE_BIN) ]; then \
		echo "Error: Binary not found at $(RELEASE_BIN). Build failed?"; \
		exit 1; \
	fi
ifeq ($(UNAME), Darwin)
	@if [ ! -f $(NETWORK_DIR)/com.tun.daemon.plist ]; then \
		echo "Error: com.tun.daemon.plist not found"; \
		exit 1; \
	fi
	sudo mkdir -p /usr/local/bin
	sudo cp $(RELEASE_BIN) /usr/local/bin/tun_daemon
	sudo chmod 755 /usr/local/bin/tun_daemon
	sudo chown root:wheel /usr/local/bin/tun_daemon

	# Create and set permissions for log files
	sudo touch /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chmod 644 /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chown root:wheel /var/log/tun_daemon.log /var/log/tun_daemon.err

	# Create and set permissions for PID file
	sudo touch /var/run/tun_daemon.pid
	sudo chmod 644 /var/run/tun_daemon.pid
	sudo chown root:wheel /var/run/tun_daemon.pid

	# Install and load the launch daemon
	sudo cp $(NETWORK_DIR)/com.tun.daemon.plist /Library/LaunchDaemons/
	sudo chown root:wheel /Library/LaunchDaemons/com.tun.daemon.plist
	sudo chmod 644 /Library/LaunchDaemons/com.tun.daemon.plist
	sudo launchctl unload /Library/LaunchDaemons/com.tun.daemon.plist 2>/dev/null || true
	sudo launchctl load -w /Library/LaunchDaemons/com.tun.daemon.plist
else ifeq ($(UNAME), Linux)
	@if [ ! -f $(NETWORK_DIR)/tun_daemon.service ]; then \
		echo "Error: tun_daemon.service not found"; \
		exit 1; \
	fi
	sudo mkdir -p /usr/local/bin
	sudo cp $(RELEASE_BIN) /usr/local/bin/tun_daemon
	sudo chmod 755 /usr/local/bin/tun_daemon
	sudo chown root:wheel /usr/local/bin/tun_daemon

	# Create and set permissions for log files
	sudo touch /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chmod 644 /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo chown root:wheel /var/log/tun_daemon.log /var/log/tun_daemon.err

	# Create and set permissions for PID file
	sudo touch /var/run/tun_daemon.pid
	sudo chmod 644 /var/run/tun_daemon.pid
	sudo chown root:wheel /var/run/tun_daemon.pid

	# Install and enable the systemd service
	sudo cp $(NETWORK_DIR)/tun_daemon.service /etc/systemd/system/
	sudo chmod 644 /etc/systemd/system/tun_daemon.service
	sudo systemctl daemon-reload
	sudo systemctl enable tun_daemon
	sudo systemctl restart tun_daemon
endif

uninstall:
ifeq ($(UNAME), Darwin)
	# First try graceful termination through launchctl
	sudo launchctl unload /Library/LaunchDaemons/com.tun.daemon.plist 2>/dev/null || true

	# If process is still running, force kill it
	@if [ -f /var/run/tun_daemon.pid ]; then \
		PID=$$(cat /var/run/tun_daemon.pid); \
		if ps -p $$PID > /dev/null; then \
			echo "Force killing daemon process..."; \
			sudo kill -9 $$PID || true; \
		fi \
	fi

	sudo rm -f /Library/LaunchDaemons/com.tun.daemon.plist
	sudo rm -f /usr/local/bin/tun_daemon
	sudo rm -f /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo rm -f /var/run/tun_daemon.pid
else ifeq ($(UNAME), Linux)
	# Stop and disable the service
	sudo systemctl stop tun_daemon || true
	sudo systemctl disable tun_daemon || true

	# If process is still running, force kill it
	@if [ -f /var/run/tun_daemon.pid ]; then \
		PID=$$(cat /var/run/tun_daemon.pid); \
		if ps -p $$PID > /dev/null; then \
			echo "Force killing daemon process..."; \
			sudo kill -9 $$PID || true; \
		fi \
	fi

	sudo rm -f /etc/systemd/system/tun_daemon.service
	sudo rm -f /usr/local/bin/tun_daemon
	sudo rm -f /var/log/tun_daemon.log /var/log/tun_daemon.err
	sudo rm -f /var/run/tun_daemon.pid
	sudo systemctl daemon-reload
endif

clean:
	cargo clean
