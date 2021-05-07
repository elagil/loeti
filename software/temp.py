import matplotlib.pyplot as plt
import numpy as np

# recorded
x = 3*np.array([350, 330, 310, 290, 250, 220])
y = np.array([330, 315, 300, 285, 250, 225])

x = np.array([190, 222, 263, 300, 325])
y = np.array([2980, 3085, 3201, 3312, 3392])

# linear regression
m, c = np.polyfit(x, y, 1)
print(1/m)
print(-c)

#plt.plot(x, y)
#plt.plot(x, c+x*m)

plt.plot(y, y-(c+x*m))
plt.xlabel("Zieltemperatur/°C")
plt.ylabel("Fehler/°C")

plt.show()
