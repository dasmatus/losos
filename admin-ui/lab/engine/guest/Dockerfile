FROM ubuntu:22.04 AS rootfs-dev
RUN apt-get update && apt-get install -y gcc make bzip2 e2fsprogs
WORKDIR /work
COPY busybox_1.36.1.orig.tar.bz2 .
RUN tar xjf busybox_1.36.1.orig.tar.bz2
WORKDIR /work/busybox-1.36.1
RUN make LDFLAGS=--static defconfig && sed -i 's/^CONFIG_TC=y/# CONFIG_TC is not set/' .config && make LDFLAGS=--static -j4
RUN mkdir -p /rootfs/bin && cp busybox /rootfs/bin/busybox && for i in $(./busybox --list) ; do ln -s busybox /rootfs/bin/$i ; done
RUN mkdir -p /rootfs/usr/share/udhcpc/ && cp ./examples/udhcp/simple.script /rootfs/usr/share/udhcpc/default.script
RUN mkdir -p /rootfs/proc /rootfs/sys /rootfs/mnt /rootfs/run /rootfs/tmp /rootfs/dev /rootfs/var /rootfs/etc /rootfs/root && mknod /rootfs/dev/null c 1 3 && chmod 666 /rootfs/dev/null
RUN ln -s /tmp/resolv.conf /rootfs/etc/resolv.conf && echo 'root:x:0:0:root:/root:/bin/sh' > /rootfs/etc/passwd && echo 'export PS1="\h:\w# "' > /rootfs/etc/profile
COPY rootfs-extra/ /rootfs/
RUN dd if=/dev/zero of=/rootfs.bin bs=1M count=6 && mke2fs -t ext4 -d /rootfs /rootfs.bin && mkdir /out && mv /rootfs.bin /out/

FROM ubuntu:22.04 AS kernel-dev
RUN apt-get update && apt-get install -y gcc make gperf flex bison bc libelf-dev libssl-dev
COPY linux /work/linux
WORKDIR /work/linux
COPY linux_x86_config .config
RUN make ARCH=x86 olddefconfig && make ARCH=x86 -j4 bzImage && mkdir /out && cp arch/x86/boot/bzImage /out/bzImage

FROM scratch
COPY --from=rootfs-dev /out/rootfs.bin /
COPY --from=kernel-dev /out/bzImage /
