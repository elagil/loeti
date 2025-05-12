import matplotlib.pyplot as plt
import numpy as np

# recorded
x = np.array([1316, 1474, 1763, 1909, 2080, 2215])
y = np.array([200, 220, 260, 280, 300, 320])

# linear regression
m, c = np.polyfit(x, y, 1)
print(m)
print(c)

plt.plot(x, y)
plt.plot(x, c+x*m)

plt.plot(y, y-(c+x*m))
plt.xlabel("Eingang (roh)")
plt.ylabel("Temperatur/°C")

plt.show()
