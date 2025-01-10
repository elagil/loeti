import matplotlib.pyplot as plt
import numpy as np

# recorded
x = 3.3 * np.array([1316, 1474, 1763, 1909, 2080, 2215]) / 4096
y = np.array([200, 220, 260, 280, 300, 320])

# linear regression
coeffs = np.polyfit(x, y, 1)
print(f"{x=}")
print(f"{coeffs=}")

a, b = coeffs
y_fit = a*x + b

plt.figure()
plt.plot(x, y)
plt.plot(x, y_fit)

plt.figure()
plt.plot(y, y-y_fit)
plt.xlabel("Eingang (roh)")
plt.ylabel("Temperatur/°C")

plt.show()
