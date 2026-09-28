# Runner image for running the CI workflow locally with act (see
# DEVELOPMENT.md, "Running CI Locally with act").
#
# act runs each job as the image's default user, so this image is shaped
# to mirror a real GitHub Actions runner. Build once from the repo root:
#
#   docker build -f scripts/act-runner.Dockerfile -t act-ubuntu .
#
FROM catthehacker/ubuntu:act-latest
# podman: /var/run must be a real directory, not a symlink to /run, or
# podman's CopyToContainer fails ("path escapes from parent",
# nektos/act#6092). Drop this line when the engine is Docker.
RUN rm -f /var/run && mkdir -p /var/run
# Non-privileged job user (mirrors the GH Actions `runner` user, uid 1001),
# with passwordless sudo for the workflow's `sudo apt-get` steps. Without
# this, act runs the job as root and snmpsim refuses to start ("Must drop
# privileges to a non-privileged user&group").
RUN useradd -u 1001 -m -s /bin/bash runner && \
    echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner && \
    chmod 440 /etc/sudoers.d/runner
# Pre-install the e2e Python deps as root so the binaries land in
# /usr/local/bin (on PATH). A pip install run as 1001 lands in
# ~/.local/bin (not on PATH) and snmpsim-test.py can't find
# snmpsim-command-responder.
RUN python3 -m pip install --break-system-packages snmpsim pysmi python-xlib
USER 1001
