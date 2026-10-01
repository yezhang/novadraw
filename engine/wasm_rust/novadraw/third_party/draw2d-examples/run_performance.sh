#!/bin/sh
set -eu

unset CDPATH
ROOT=$(cd -- "$(dirname -- "$0")" && pwd)
SWT_VERSION=${SWT_VERSION:-3.135.0}
REPORT_DIR=${REPORT_DIR:-"$ROOT/../../target/verification/reports/ga2-draw2d"}
WARMUP=${WARMUP:-5}
SAMPLES=${SAMPLES:-30}
UNAME_S=$(uname -s)
UNAME_M=$(uname -m)

case "$UNAME_S:$UNAME_M" in
  Darwin:arm64)
    SWT_ARTIFACT=org.eclipse.swt.cocoa.macosx.aarch64
    FIRST_THREAD=-XstartOnFirstThread
    NATIVE_GLOB='*.jnilib'
    ;;
  Darwin:x86_64)
    SWT_ARTIFACT=org.eclipse.swt.cocoa.macosx.x86_64
    FIRST_THREAD=-XstartOnFirstThread
    NATIVE_GLOB='*.jnilib'
    ;;
  Linux:x86_64)
    SWT_ARTIFACT=org.eclipse.swt.gtk.linux.x86_64
    FIRST_THREAD=
    NATIVE_GLOB='*.so'
    ;;
  Linux:aarch64)
    SWT_ARTIFACT=org.eclipse.swt.gtk.linux.aarch64
    FIRST_THREAD=
    NATIVE_GLOB='*.so'
    ;;
  *)
    echo "unsupported Draw2D performance host: $UNAME_S $UNAME_M" >&2
    exit 2
    ;;
esac

DEPS_DIR="$ROOT/target/performance-deps"
CLASSES_DIR="$ROOT/target/performance-classes"
NATIVE_DIR="$DEPS_DIR/native-$SWT_ARTIFACT-$SWT_VERSION"
DRAW2D_JAR="$ROOT/lib/org.eclipse.draw2d-3.22.0-SNAPSHOT.jar"
SWT_JAR="$DEPS_DIR/$SWT_ARTIFACT-$SWT_VERSION.jar"
SWT_URL="https://repo1.maven.org/maven2/org/eclipse/platform/$SWT_ARTIFACT/$SWT_VERSION/$SWT_ARTIFACT-$SWT_VERSION.jar"
SOURCE="$ROOT/src/main/java/org/example/draw2d/performance/Draw2dPerformanceRunner.java"

mkdir -p "$DEPS_DIR" "$CLASSES_DIR" "$REPORT_DIR"
if [ ! -f "$SWT_JAR" ]; then
  echo "Downloading SWT $SWT_ARTIFACT $SWT_VERSION"
  curl -fsSL "$SWT_URL" -o "$SWT_JAR"
fi
if [ ! -d "$NATIVE_DIR" ]; then
  mkdir -p "$NATIVE_DIR"
  unzip -jq "$SWT_JAR" "$NATIVE_GLOB" -d "$NATIVE_DIR"
fi

javac -encoding UTF-8 --release 17 \
  -cp "$DRAW2D_JAR:$SWT_JAR" \
  -d "$CLASSES_DIR" \
  "$SOURCE"

SCENARIOS="
wide_tree_full_paint_4096
deep_tree_full_paint_1000
deep_tree_full_paint_10000
deep_tree_validate_1000
deep_tree_validate_10000
local_update_full_paint_1pct_4096
full_update_full_paint_100pct_4096
"

for SCENARIO in $SCENARIOS; do
  TIME_REPORT="$REPORT_DIR/.$SCENARIO.time"
  if [ "$UNAME_S" = "Darwin" ]; then
    TIME_ARGS="-l"
    RSS_METHOD=bsd-time-maximum-resident-set-size-bytes
  else
    TIME_ARGS="-v"
    RSS_METHOD=gnu-time-maximum-resident-set-size-kib
  fi
  /usr/bin/time $TIME_ARGS -o "$TIME_REPORT" \
    java $FIRST_THREAD -Xss64m -Xms128m -Xmx1g \
    -Ddraw2d.jar="$DRAW2D_JAR" \
    -Dswt.jar="$SWT_JAR" \
    -Dswt.library.path="$NATIVE_DIR" \
    -cp "$CLASSES_DIR:$DRAW2D_JAR:$SWT_JAR" \
    org.example.draw2d.performance.Draw2dPerformanceRunner \
    "--scenario=$SCENARIO" \
    "--warmup=$WARMUP" \
    "--samples=$SAMPLES" \
    "--report=$REPORT_DIR/$SCENARIO.json"

  if [ "$UNAME_S" = "Darwin" ]; then
    PEAK_RSS=$(awk '/maximum resident set size/ { print $1 }' "$TIME_REPORT")
  else
    PEAK_RSS_KIB=$(awk -F: '/Maximum resident set size/ { gsub(/^[ \t]+/, "", $2); print $2 }' "$TIME_REPORT")
    PEAK_RSS=$((PEAK_RSS_KIB * 1024))
  fi
  jq \
    --arg method "$RSS_METHOD" \
    --argjson peak "$PEAK_RSS" \
    '.measurement_scope.process_peak_rss = true
     | .scenario.memory.process_peak_rss_method = $method
     | .scenario.memory.process_peak_rss_bytes = $peak' \
    "$REPORT_DIR/$SCENARIO.json" > "$REPORT_DIR/.$SCENARIO.json"
  mv "$REPORT_DIR/.$SCENARIO.json" "$REPORT_DIR/$SCENARIO.json"
  rm "$TIME_REPORT"
done

echo "REPORT_DIR $REPORT_DIR"
