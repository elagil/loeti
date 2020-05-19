import matplotlib.pyplot as plt
import struct
import numpy as np

from scipy.signal import butter, filtfilt

# 0.3 A -> 6 W, 0.5 Hz

# Filter requirements.
fs = 0.5  # sample rate, Hz
T = 110 / fs  # Sample Period
cutoff = 0.05  # desired cutoff frequency of the filter, Hz ,      slightly higher than actual 1.2 Hz
nyq = 0.5 * fs  # Nyquist Frequency
order = 2       # sin wave can be approx represented as quadratic
n = int(T * fs)  # total number of samples


def butter_lowpass_filter(data, cutoff, fs, order):
    normal_cutoff = cutoff / nyq
    # Get the filter coefficients
    b, a = butter(order, normal_cutoff, btype='low', analog=False)
    y = filtfilt(b, a, data)
    return y


f = open("file.bin", "rb")
data = f.read()
f.close()
data_unpacked = np.array(struct.unpack('110d', data))
dd = np.diff(data_unpacked - data_unpacked[0])

# Filter the data
lp = butter_lowpass_filter(dd, cutoff, fs, order)

plt.figure()
plt.plot(lp)

plt.figure()
plt.plot(data_unpacked - data_unpacked[0])

for i, d in enumerate(data_unpacked):
    if i == 0:
        inp = 0
    else:
        inp = 0.1

    print("%f, %f, %f" % (i/fs, inp, d))

plt.show()
