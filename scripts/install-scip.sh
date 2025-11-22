#!/bin/bash
# Install SCIP Optimization Suite
# Supports Ubuntu, macOS, and source builds
set -e

SCIP_VERSION="${SCIP_VERSION:-9.2.4}"
SCIP_VERSION_NUM="${SCIP_VERSION//./}"  # Remove dots: 9.2.4 -> 924
FORCE_SOURCE="${FORCE_SOURCE:-false}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v lsb_release &> /dev/null; then
            DISTRO=$(lsb_release -is)
            VERSION=$(lsb_release -rs | cut -d. -f1)
            echo "linux-${DISTRO,,}-${VERSION}"
        else
            echo "linux-unknown"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unsupported"
    fi
}

check_scip_installed() {
    if pkg-config --exists scip 2>/dev/null; then
        local installed_version
        installed_version=$(pkg-config --modversion scip)
        log_info "SCIP is already installed: version ${installed_version}"

        if [[ "${installed_version}" == "${SCIP_VERSION}" ]]; then
            return 0
        else
            log_warn "Installed version ${installed_version} differs from requested ${SCIP_VERSION}"
            return 1
        fi
    fi
    return 1
}

install_scip_ubuntu() {
    local ubuntu_version=$1
    local deb_file="SCIPOptSuite-${SCIP_VERSION}-Linux-ubuntu${ubuntu_version}.deb"
    local deb_url="https://github.com/scipopt/scip/releases/download/v${SCIP_VERSION_NUM}/${deb_file}"

    log_info "Installing SCIP ${SCIP_VERSION} for Ubuntu ${ubuntu_version}..."

    # Download
    log_info "Downloading ${deb_file}..."
    log_info "URL: ${deb_url}"
    if ! wget --show-progress "$deb_url" -O "/tmp/${deb_file}"; then
        log_error "Failed to download ${deb_url}"
        return 1
    fi

    # Install
    log_info "Installing package..."
    if dpkg -i "/tmp/${deb_file}" 2>/dev/null; then
        log_info "Package installed successfully"
    else
        log_warn "dpkg reported issues, attempting to fix dependencies..."
        apt-get install -f -y
    fi

    rm "/tmp/${deb_file}"
    ldconfig 2>/dev/null || true

    return 0
}

install_scip_macos() {
    log_info "Installing SCIP via Homebrew..."

    if ! command -v brew &> /dev/null; then
        log_error "Homebrew not found. Please install from https://brew.sh"
        return 1
    fi

    brew install scip
    return 0
}

install_scip_source() {
    log_info "Building SCIP ${SCIP_VERSION} from source..."

    # Install dependencies
    log_info "Installing build dependencies..."
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo apt-get update
        sudo apt-get install -y \
            wget cmake g++ m4 xz-utils \
            libgmp-dev unzip zlib1g-dev \
            libboost-program-options-dev \
            libboost-serialization-dev \
            libboost-regex-dev \
            libboost-iostreams-dev \
            libtbb-dev libreadline-dev \
            pkg-config git liblapack-dev \
            libgsl-dev flex bison \
            libcliquer-dev gfortran \
            file dpkg-dev libopenblas-dev
    fi

    # Download
    local tarball="https://github.com/scipopt/scip/archive/refs/tags/v${SCIP_VERSION_NUM}.tar.gz"
    log_info "Downloading source from ${tarball}..."
    wget -q --show-progress "$tarball" -O /tmp/scip.tar.gz

    # Extract
    log_info "Extracting source..."
    mkdir -p /tmp/scip-build
    tar -xzf /tmp/scip.tar.gz -C /tmp/scip-build --strip-components=1
    cd /tmp/scip-build

    # Build
    log_info "Configuring build..."
    mkdir -p build && cd build
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=/usr/local

    log_info "Building (this may take 20-30 minutes)..."
    make -j$(nproc)

    # Install
    log_info "Installing..."
    sudo make install
    sudo ldconfig

    # Cleanup
    cd /
    rm -rf /tmp/scip-build /tmp/scip.tar.gz

    return 0
}

verify_installation() {
    log_info "Verifying SCIP installation..."

    if ! pkg-config --exists scip; then
        log_error "pkg-config cannot find SCIP"
        return 1
    fi

    local installed_version
    installed_version=$(pkg-config --modversion scip)
    log_info "SCIP version: ${installed_version}"

    # Check library
    if ldconfig -p | grep -q libscip; then
        log_info "SCIP library found in system library path"
    else
        log_warn "SCIP library not in ldconfig cache"
    fi

    # Print configuration
    log_info "SCIP configuration:"
    echo "  Version:  $(pkg-config --modversion scip)"
    echo "  CFLAGS:   $(pkg-config --cflags scip)"
    echo "  LIBS:     $(pkg-config --libs scip)"

    return 0
}

main() {
    log_info "SCIP Installation Script"
    log_info "Target version: ${SCIP_VERSION}"

    # Check if already installed
    if [[ "${FORCE_SOURCE}" != "true" ]] && check_scip_installed; then
        log_info "SCIP ${SCIP_VERSION} is already installed. Skipping."
        verify_installation
        exit 0
    fi

    # Detect platform
    platform=$(detect_platform)
    log_info "Detected platform: ${platform}"

    # Install based on platform
    case "$platform" in
        linux-ubuntu-24)
            if [[ "${FORCE_SOURCE}" == "true" ]]; then
                log_info "FORCE_SOURCE=true, building from source"
                install_scip_source || exit 1
            else
                install_scip_ubuntu 24 || {
                    log_warn ".deb installation failed, falling back to source build"
                    install_scip_source || exit 1
                }
            fi
            ;;
        linux-ubuntu-22)
            if [[ "${FORCE_SOURCE}" == "true" ]]; then
                install_scip_source || exit 1
            else
                install_scip_ubuntu 22 || install_scip_source || exit 1
            fi
            ;;
        linux-ubuntu-20)
            if [[ "${FORCE_SOURCE}" == "true" ]]; then
                install_scip_source || exit 1
            else
                install_scip_ubuntu 20 || install_scip_source || exit 1
            fi
            ;;
        macos)
            install_scip_macos || {
                log_error "macOS installation failed"
                exit 1
            }
            ;;
        *)
            log_warn "Unsupported platform: $platform"
            log_info "Attempting source build..."
            install_scip_source || {
                log_error "Source build failed for unsupported platform"
                exit 1
            }
            ;;
    esac

    # Verify
    verify_installation || {
        log_error "Installation verification failed"
        exit 1
    }

    log_info "✓ SCIP ${SCIP_VERSION} installed successfully!"
}

# Run main function
main "$@"
