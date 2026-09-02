# Audiover

Linux (PipeWire / PulseAudio) için gerçek zamanlı ses dönüştürücü (voice changer) ve ses tahtası (soundboard) uygulaması.

---

## Özellikler

### Ses Dönüştürücü (DSP)
- **Pitch Shifter:** Granüler sentez ile perde kaydırma (-12 / +12 semiton).
- **Robot / Ring Modulator:** Taşıyıcı frekansı ayarlanabilir ring modülasyonu (50 Hz - 600 Hz).
- **Telsiz Filtresi:** 2. derece Butterworth bandpass filtre (300 Hz – 3400 Hz) ve harmonik distorsiyon.
- **Distortion:** Tanjant hiperbolik (`tanh`) tabanlı analog doygunluk.
- **Reverb:** 4 paralel tarak ve 2 tüm-geçiren filtreli Schroeder yankı efekti.
- **Chorus:** LFO modülasyonlu uzamsal gecikme efekti.
- **Noise Gate:** Yumuşak geçişli (soft-knee) gürültü kapısı.
- **Ön Ayarlar ve DSP Ayarları:** Hazır ve özel efekt profilleri oluşturma ve düzenleme.
- **Bypass:** Efekt zincirini devre dışı bırakıp ham mikrofon sesine dönme.

### Soundboard
- **Format Desteği:** `.mp3`, `.wav`, `.ogg`, `.flac`, `.m4a`, `.mp4` (yalnızca ses akışı).
- **Bellek Üzerinden Oynatma:** Sesler RAM'e 48 kHz 32-bit float stereo olarak yüklenir ve gecikmesiz tetiklenir.
- **Kanal Kontrolleri:** Bağımsız ses seviyesi, döngü (loop) ve durdurma kontrolleri.
- **Toplu Durdurma:** Çalan tüm sesleri tek tuşla durdurma.

### Sanal Ses Yönlendirme (PipeWire / PulseAudio)
- **Sanal Aygıt Yönetimi:** `Audiover_Sink` ve `Audiover_Mic` (`module-remap-source`) aygıtları ile otomatik miks yönetimi.
- **Uygulama Entegrasyonu:** Discord, OBS Studio, Steam ve oyunlara dönüştürülmüş mikrofon ve ses tahtası çıkışını aktarma.
- **Geri Bildirim Koruması:** Fiziksel ve sanal aygıtlar arasında döngü (loopback) oluşmasını engelleme.
- **Otomatik Temizleme:** Uygulama kapandığında oluşturulan sanal modülleri sistemden kaldırma.

### Canlı Dinleme ve Giriş Yönetimi
- **Canlı Dinleme (Hear Myself):** Dönüştürülen ses ve soundboard çıkışını kulaklıktan dinleme.
- **Global Kısayollar:** Wayland ve X11 ortamlarında `/dev/input` üzerinden arka planda kısayol desteği.

---

## Mimari

```
                  ┌─────────────────────────────┐
                  │       Fiziksel Mikrofon     │
                  └──────────────┬──────────────┘
                                 │ (48kHz Float32)
                                 ▼
                  ┌─────────────────────────────┐
                  │    DSP Efekt Zinciri        │
                  │  - Noise Gate               │
                  │  - Pitch Shifter            │
                  │  - Ring Modulator           │
                  │  - Bandpass Filter          │
                  │  - Distortion / Saturator   │
                  │  - Schroeder Reverb / Chorus│
                  └──────────────┬──────────────┘
                                 │
┌───────────────────────┐        │ (İşlenmiş Ses)
│   Soundboard Motoru   │        │
│   (RAM 48kHz Stereo)  ├────────┼────────────────────────────┐
└───────────┬───────────┘        │                            │
            │ (SFX)              ▼                            ▼
            │             ┌──────────────┐             ┌──────────────┐
            └────────────►│ Master Mikser│             │ Monitör      │
                          │  + Limiter   │             │ (Kulaklık)   │
                          └──────┬───────┘             └──────────────┘
                                 │
                                 ▼
                  ┌─────────────────────────────┐
                  │  Audiover_Sink (Null Sink)  │
                  └──────────────┬──────────────┘
                                 │
                                 ▼
                  ┌─────────────────────────────┐
                  │ Audiover_Virtual_Microphone │
                  │    (module-remap-source)    │
                  └──────────────┬──────────────┘
                                 │
                    ┌────────────┴────────────┐
                    ▼                         ▼
             [Discord / OBS]          [Oyunlar / Diğer]
```

### Teknik Parametreler
- **Örnekleme Hızı:** 48.000 Hz
- **Blok Boyutu:** 256 frame (~5.33 ms gecikme)
- **Ses Formatı:** 32-bit float (`float32`, [-1.0, 1.0])
- **Kırpılma Koruması:** Master çıkışta yumuşak sınırlayıcı (soft limiter)
- **Girdi Takibi:** Linux `/dev/input/event*` standardı ile global kısayol takibi

---

## Kurulum ve Çalıştırma

### Gereksinimler
- Linux (Fedora, Arch, Ubuntu vb.)
- PipeWire (`pipewire-pulse`) veya PulseAudio
- `pulseaudio-utils` (`pactl`), `ffmpeg`
- Python 3.10+

### Kurulum
```bash
chmod +x setup_env.sh run.sh
./setup_env.sh
```

### Global Kısayol İzni (Opsiyonel)
Wayland üzerinde global kısayolların `/dev/input` üzerinden dinlenebilmesi için kullanıcının `input` grubunda olması gerekir:
```bash
sudo usermod -aG input $USER
```
*(Komutun geçerli olması için oturumu kapatıp yeniden açın).*

### Başlatma
```bash
./run.sh
```

---

## Ses Yapılandırması

1. Audiover uygulamasını başlatın.
2. Hedef uygulamada (Discord, OBS Studio vb.) mikrofon / giriş aygıtı olarak **Audiover_Virtual_Microphone** (veya `Audiover_Mic`) seçin.
3. Çıkış aygıtı olarak kulaklığınızı seçin.

---

## Varsayılan Kısayollar

| Tuş | Eylem |
|---|---|
| `F8` | Canlı dinleme (Hear Myself) aç / kapat |
| `F9` | Mikrofonu sustur (Mute) / aç |
| `F10` | DSP efektlerini devre dışı bırak (Bypass) / etkinleştir |
| `F11` | Çalan tüm soundboard seslerini durdur |
| `1` | Airhorn |
| `2` | Level Up |
| `3` | Cyber Alert |

*Kısayollar uygulama arayüzünden özelleştirilebilir.*

---

## Lisans

MIT
