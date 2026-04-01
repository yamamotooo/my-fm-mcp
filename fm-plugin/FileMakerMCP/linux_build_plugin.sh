#!/bin/bash
#
# Copyright © 2023 Claris International Inc. All rights reserved.
#


set -o pipefail

#
# Prompt for password for later sudo calls
#
echo "NOTE: this build script needs to run some commands as root (using sudo)"
read -s -p "Please enter your local machine password: " sudoPW
echo

InstallPackages()
{
	echo "=== $(FormatDate) ${FUNCNAME[0]}()"
	OS_VERSION=$(cat /etc/issue | head -n 1 | cut -c 8-9)
	apt_packages=( 					\
						'libc6'		\
						'make'		\
						'cmake'		\
		)

	if [[ "$OS_VERSION" == "20" ]]; then
		# fundamental C++ compiler packages and Cmake
		apt_packages+=('clang-12')
		apt_packages+=('libc++-12-dev')
		apt_packages+=('libc++abi-12-dev')
	fi
	if [[ "$OS_VERSION" == "22" ]]; then
		# fundamental C++ compiler packages and Cmake
		apt_packages+=('clang-14')
		apt_packages+=('libc++-14-dev')
		apt_packages+=('libc++abi-14-dev')
		# required on Parallels for arm64
		apt_packages+=('libstdc++-12-dev')
	fi
	if [[ "$OS_VERSION" == "24" ]]; then
		# fundamental C++ compiler packages and Cmake
		apt_packages+=('clang-16')
		apt_packages+=('libc++-16-dev')
		apt_packages+=('libc++abi-16-dev')
		# required on Parallels for arm64
		apt_packages+=('libstdc++-12-dev')
	fi

	for i in ${apt_packages[@]}; do
		pkg="$i"
		apt list --installed 2>&1 | grep -w "$pkg" > /dev/null
		errStatus=$?
		if [[ $errStatus -ne 0 ]]; then
			echo "=== $(FormatDate) Package $pkg is not installed, run sudo apt install $pkg ..." | tee -a "$LOG_FILE"
			echo "$sudoPW" | sudo -S -- apt install -y "$pkg" 2>&1 | tee -a "$LOG_FILE"
			errStatus=$?
			[[ $errStatus -ne 0 ]] && apt list "$pkg" | tee -a "$LOG_FILE" && die "error: failed to install package $pkg, error: $errStatus !"
		else
			install_packages+=("$pkg") 
		fi
	done

	echo "=== $(FormatDate) ${FUNCNAME[0]}() returns $errStatus"
}


SCRIPT_NAME=$(basename "$0")
DIR_NAME=$(dirname "$0")
SCRIPT_DIR=$(cd "$DIR_NAME" && pwd)

#
# CMake uses the root directory of the workspace
#
WORKSPACE=$(cd "$SCRIPT_DIR" && pwd)

BUILD_METHOD="build"
CONFIG_TYPE="Release"
LOG_DIR="$SCRIPT_DIR"

FormatDate()
{
	date "+%m/%d/%H:%M:%S"
}

usage()
{
	cat << EOF
Usage:
  $0  [-b build|rebuild] [-c Debug|Release] [-l logDirectory] [-s sourceDirectory] [-v buildVersion]

This script will check the CMakeLists.txt and build FileMaker plugin based on it.

Options:
 -b	Build type.
		build - configure and build workspace, it supports incremental compilation.
		rebuild - clean workspace configuration before building, it will delete the build directory.
 -c	Build configuration.
		Debug - Disable optimization.
		Release - Optimize code with O3.
 -h	Display this help.
 -l	Specify Log directory.
 -s	Source directory. Specify the workspace directory where the top level CMakeLists.txt placed.
	The default is the directory where the script is located.
 -v	Build version. Specify project version.

e.g.
./linux_build_plugin.sh
./linux_build_plugin.sh -b rebuild -c Release -s /home/fmserver/PluginSDK/MiniExample -l /home/fmserver/PluginSDK/MiniExample -v 1.0.0

EOF

	exit 0
}

BuildPlugin()
{
	echo "=== $(FormatDate) ${FUNCNAME[0]}()"
	local err_code=0
	local result=0

	if [[ "$BUILD_METHOD" == "rebuild" ]]; then
		echo "=== $(FormatDate) Delete cmake cache in $WORKSPACE/build" | tee -a "$LOG_FILE"
		rm -rf "$WORKSPACE/build"
		if [[ -d "$WORKSPACE/build" ]]; then
			echo "$(FormatDate) ${FUNCNAME[0]}() error: failed to delete cmake cache in $WORKSPACE/build" | tee -a "$LOG_FILE"
			return 1
		fi
	fi
	
	if [[ ! -d "$WORKSPACE/build" ]]; then
		echo "=== $(FormatDate) $WORKSPACE/build does not exist, run cmake -D CMAKE_BUILD_TYPE=$CONFIG_TYPE -D CMAKE_PROJECT_VERSION=$BUILD_VERSION -B $WORKSPACE/build -G 'Unix Makefiles'" -S "$WORKSPACE" | tee -a "$LOG_FILE"
		cmake -D CMAKE_BUILD_TYPE="$CONFIG_TYPE" -D CMAKE_PROJECT_VERSION="$BUILD_VERSION" -B "$WORKSPACE/build" -G "Unix Makefiles" -S "$WORKSPACE" >> "$LOG_FILE" 2>&1
		result=$?
		if [[ $result -ne 0 ]]; then
			err_code=1
		fi
	fi

	if [[ -d "$WORKSPACE/build" ]]; then
		echo "=== $(FormatDate) Run cmake --build $WORKSPACE/build --config $CONFIG_TYPE --target clean -v" | tee -a "$LOG_FILE"
		cmake --build "$WORKSPACE/build" --config "$CONFIG_TYPE" --target clean -v >> "$LOG_FILE" 2>&1
		echo "=== $(FormatDate) Run cmake --build $WORKSPACE/build --config $CONFIG_TYPE --target all --parallel $USABLE_CPU -v" | tee -a "$LOG_FILE"
		cmake --build "$WORKSPACE/build" --config "$CONFIG_TYPE" --target all --parallel "$USABLE_CPU" -v >> "$LOG_FILE" 2>&1
		result=$?
		[[ $result -ne 0 ]] && err_code=2
	
		if [[ $err_code -ne 0 ]]; then
			echo "=== $(FormatDate) ${FUNCNAME[0]}() failed to build FMMiniPlugin, error: $err_code, log file: $LOG_FILE" | tee -a "$LOG_FILE"
			return $err_code
		else
			echo "=== $(FormatDate) ${FUNCNAME[0]}() build FMMiniPlugin successfully." | tee -a "$LOG_FILE"
		fi
	else
		echo "=== $(FormatDate) ${FUNCNAME[0]}() $WORKSPACE/build is not created." | tee -a "$LOG_FILE"
		return $err_code
	fi
	echo "=== $(FormatDate) ${FUNCNAME[0]}() returns $result"
	return $result
}

while getopts b:c:hl:s:v: opt
do
	case $opt in
	b )
		case "$OPTARG" in
		build | rebuild )
			BUILD_METHOD=$OPTARG
			;;
		* )
			echo "Unknown build type $OPTARG"
			exit 1
			;;
		esac
		;;
	c )	
		case "$OPTARG" in
		Release | release )
			CONFIG_TYPE="Release"
			;;
		Debug | debug )
			CONFIG_TYPE="Debug"
			;;
		* )
			echo "Unknown configuration $OPTARG"
			exit 1
			;;
		esac
		;;
	h ) 
		usage
		;;
	l )	  
		LOG_DIR="$OPTARG"
		;;
	s )
		WORKSPACE="$OPTARG"
		;;
	v )
		BUILD_VERSION=$OPTARG
		;;
	* ) # capture single argument without provide value or invalid argument
		usage "	 "
		;;
	esac
done

shift $((OPTIND-1))

if [[ ! -d "$LOG_DIR" ]]; then
	if [[ $(mkdir -p "$LOG_DIR") ]]; then 
		echo "$(FormatDate) error: Failed to create log directory: $LOG_DIR."
		exit 1
	fi
fi

LOG_FILE="$LOG_DIR/Build_FMMiniPlugin_${CONFIG_TYPE}.log"
echo "" > ${LOG_FILE}	# clear the log
NUM_CPU=$(nproc --all)
USABLE_CPU=$(( (NUM_CPU*4)/5 ))
ERROR=0
echo "=== $SCRIPT_NAME, build type: $BUILD_METHOD, config type: $CONFIG_TYPE, log file: $LOG_FILE, workspace: $WORKSPACE, build version: $BUILD_VERSION, total number of processors: $NUM_CPU, number of processors to be used: $USABLE_CPU " | tee -a "$LOG_FILE"

InstallPackages
BuildPlugin
ERROR=$?
if [[ $ERROR -ne 0 ]]; then
	echo "=== $(FormatDate) Failed to build FMMiniPlugin, error: $ERROR" | tee -a "$LOG_FILE"
	exit $ERROR
else
	echo "=== $(FormatDate) Build FMMiniPlugin finished" | tee -a "$LOG_FILE"
	exit 0
fi
