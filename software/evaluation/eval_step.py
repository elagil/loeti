import pickle
import matplotlib.pyplot as plt
import numpy as np
import scipy.optimize as so


def pt1(t, K, T, off):
    return K*(1-np.exp(-(t)/T)) + off


files = ["step_bent.pkl", "step_chisel.pkl", "step_bevel.pkl"]

for fname in files:
    f = open(fname, "rb")
    data = np.array(pickle.load(f))
    powers = data[2]

    start = np.argmax(powers > 0)

    times = data[0][start:]
    temperatures = data[1][start:]

    times -= times[0]
    f.close()

    plt.plot(times, temperatures, label=fname)

    fit, cov = so.curve_fit(pt1, times, temperatures,
                            bounds=(0, [1000, 100, 100]))
    plt.plot(times, pt1(times, fit[0], fit[1], fit[2]))
    print(fit)


plt.legend()
plt.show()
