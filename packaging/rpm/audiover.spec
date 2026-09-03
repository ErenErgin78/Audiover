Name:           audiover_rust
Version:        1.0.0
Release:        1%{?dist}
Summary:        Real-Time Voice Changer & Soundboard Engine for Linux / PipeWire
License:        MIT
URL:            https://github.com/ErenErgin78/Audiover
BuildArch:      x86_64
AutoReqProv:    no
Requires:       pulseaudio-utils, /usr/bin/ffmpeg, webkit2gtk4.1, alsa-lib

%define debug_package %{nil}
%define _build_id_links none
%define __strip /bin/true
%define __os_install_post %{nil}
%define __check_files %{nil}
%define _binary_payload w3T0.zstdio

%description
Audiover is a high-performance real-time DSP voice changer and multi-channel
soundboard engine written in Rust with a React interface, designed for modern
Linux systems running PipeWire / PulseAudio.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}/opt/audiover
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/scalable/apps
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps
mkdir -p %{buildroot}/usr/share/pixmaps

# Copy application files to /opt/audiover
cp -a %{_sourcedir}/opt/audiover/* %{buildroot}/opt/audiover/

# Copy launcher to /usr/bin
cp -a %{_sourcedir}/usr/bin/audiover %{buildroot}/usr/bin/audiover
chmod +x %{buildroot}/usr/bin/audiover

# Copy desktop entry
cp -a %{_sourcedir}/usr/share/applications/audiover.desktop %{buildroot}/usr/share/applications/audiover.desktop

# Copy icons
if [ -f "%{_sourcedir}/usr/share/icons/hicolor/scalable/apps/audiover.svg" ]; then
    cp -a %{_sourcedir}/usr/share/icons/hicolor/scalable/apps/audiover.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/audiover.svg
fi
if [ -f "%{_sourcedir}/usr/share/icons/hicolor/256x256/apps/audiover.png" ]; then
    cp -a %{_sourcedir}/usr/share/icons/hicolor/256x256/apps/audiover.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/audiover.png
fi
if [ -f "%{_sourcedir}/usr/share/pixmaps/audiover.png" ]; then
    cp -a %{_sourcedir}/usr/share/pixmaps/audiover.png %{buildroot}/usr/share/pixmaps/audiover.png
fi

%post
/usr/bin/update-desktop-database &> /dev/null || :
/usr/bin/gtk-update-icon-cache -f -t %{_datadir}/icons/hicolor &> /dev/null || :

%postun
/usr/bin/update-desktop-database &> /dev/null || :
/usr/bin/gtk-update-icon-cache -f -t %{_datadir}/icons/hicolor &> /dev/null || :

%files
/opt/audiover
/usr/bin/audiover
/usr/share/applications/audiover.desktop
%{_datadir}/icons/hicolor/*/apps/audiover.*
%{_datadir}/pixmaps/audiover.png

%changelog
* Wed Sep 02 2026 Eren Ergin <erenergin78@github.com> - 1.0.0-1
- Complete rewrite in Rust / Tauri with React UI
