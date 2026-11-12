# COPR's make_srpm entry point:
#   make -f .copr/Makefile srpm outdir=<dir> spec=<ignored>
# Run as root inside COPR's mock chroot, it installs what it needs; run by
# hand, it installs nothing, so have git, cargo and rpm-build already.
#
# The chroot starts without git, so nothing below may run git until the
# install is done: `srpm` installs, then runs `build` in a second make.

.PHONY: srpm build

srpm:
	@test -n "$(outdir)" || { echo "outdir is required" >&2; exit 2; }
	if [ "$$(id -u)" = 0 ]; then dnf -y install git cargo rpm-build; fi
	$(MAKE) -f $(abspath $(lastword $(MAKEFILE_LIST))) build outdir=$(outdir)

ifneq ($(filter build,$(MAKECMDGOALS)),)
ROOT     := $(shell git rev-parse --show-toplevel)
SPEC     := $(ROOT)/packaging/fedora/zendo.spec
COMMIT   := $(shell git -C $(ROOT) rev-parse HEAD)
SHORT    := $(shell git -C $(ROOT) rev-parse --short=7 HEAD)
SNAPDATE := $(shell git -C $(ROOT) show -s --format=%cd --date=format:%Y%m%d HEAD)
WORK     := $(shell mktemp -d)
endif

build:
	mkdir -p $(WORK)/sources $(WORK)/vendor-src
	git -C $(ROOT) archive --prefix=zendo-$(COMMIT)/ -o $(WORK)/sources/zendo-$(SHORT).tar.gz $(COMMIT)
	git -C $(ROOT) archive --prefix=zendo-$(COMMIT)/ $(COMMIT) | tar -x -C $(WORK)/vendor-src
	cd $(WORK)/vendor-src/zendo-$(COMMIT) && cargo vendor --locked vendor >/dev/null
	tar -C $(WORK)/vendor-src/zendo-$(COMMIT) -czf $(WORK)/sources/zendo-$(SHORT)-vendor.tar.gz vendor
	mkdir -p $(outdir)
	sed -e 's/^\(%{!?commit:%global commit \)[0-9a-f]*}/\1$(COMMIT)}/' \
	    -e 's/^\(%{!?shortcommit:%global shortcommit \)[0-9a-f]*}/\1$(SHORT)}/' \
	    -e 's/^\(%{!?snapdate:%global snapdate \)[0-9]*}/\1$(SNAPDATE)}/' \
	    $(SPEC) > $(WORK)/zendo.spec
	grep -q "$(COMMIT)" $(WORK)/zendo.spec
	rpmbuild -bs $(WORK)/zendo.spec \
	  --define "_sourcedir $(WORK)/sources" \
	  --define "_srcrpmdir $(outdir)"
	rm -rf $(WORK)
