Name:           audiover
Version:        1.0.0
Release:        3%{?dist}
Summary:        Real-Time Voice Changer & Soundboard Engine for Linux / PipeWire
License:        MIT
URL:            https://github.com/ErenErgin78/Audiover
BuildArch:      x86_64
AutoReqProv:    no
Requires:       pulseaudio-utils, /usr/bin/ffmpeg, zenity, python3

%define debug_package %{nil}
%define _build_id_links none
%define __strip /bin/true
%define __os_install_post %{nil}
%define __check_files %{nil}
%define __brp_python_bytecompile %{nil}
%define _binary_payload w3T0.zstdio

%description
Audiover is a real-time DSP voice changer and multi-channel soundboard
engine designed for modern Linux systems running PipeWire / PulseAudio.

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
cp -a %{_sourcedir}/usr/share/icons/hicolor/scalable/apps/audiover.svg %{buildroot}/usr/share/icons/hicolor/scalable/apps/audiover.svg
cp -a %{_sourcedir}/usr/share/icons/hicolor/256x256/apps/audiover.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/audiover.png
cp -a %{_sourcedir}/usr/share/pixmaps/audiover.png %{buildroot}/usr/share/pixmaps/audiover.png

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
/usr/share/icons/hicolor/scalable/apps/audiover.svg
/usr/share/icons/hicolor/256x256/apps/audiover.png
/usr/share/pixmaps/audiover.png

%changelog
* Sun Aug 16 2026 Eren Ergin <erenergin78@github.com> - 1.0.0-1
- Initial RPM release for Audiover
