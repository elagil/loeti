import matplotlib.pyplot as plt
import struct
import numpy as np
from scipy.optimize import curve_fit

from scipy.signal import butter, filtfilt

# 0.3 A -> 6 W, 0.5 Hz


def func(t, a, b):
    return a*(1-np.exp(-t/b))


N = 384
# Filter requirements.
fs = 5  # sample rate, Hz
T = N / fs  # Sample Period
cutoff = 1  # desired cutoff frequency of the filter, Hz ,      slightly higher than actual 1.2 Hz
nyq = 0.5 * fs  # Nyquist Frequency
order = 2       # sin wave can be approx represented as quadratic
n = int(T * fs)  # total number of samples

f = open("file.bin", "rb")
data = f.read()
f.close()
data_unpacked = np.array(struct.unpack('384H', data))

data_unpacked = data_unpacked / 10

dcomp = data_unpacked - data_unpacked[0]
t = np.linspace(0, T, N, endpoint=False)

popt, pcov = curve_fit(func, t[:-19], dcomp[19:])

y = func(t, popt[0], popt[1])

plt.figure()
plt.plot(t[:-19], dcomp[19:])
plt.plot(t, y)

plt.show()
