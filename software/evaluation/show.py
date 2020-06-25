import numpy as np
import serial
import sys
import concurrent.futures as cf

from PyQt5 import QtGui
from PyQt5.QtWidgets import QApplication, QMainWindow
import pyqtgraph as pg


def getNumber(raw, factor):
    return float(raw[-5:])/factor


def acquire(plotTemp, plotPower):
    s = serial.Serial('COM5', 115200, timeout=1)

    temperatures = []
    powers = []

    tempPlot = plotTemp.plot()
    powerPlot = plotPower.plot()
    current = 0
    voltage = 0
    power = 0

    while True:
        res = s.readline().decode("utf-8")
        res = res[:-1]

        if res[0] == "A":
            current = getNumber(res, 1000)

        elif res[0] == "V":
            voltage = getNumber(res, 1000)

        elif res[0] == "C":
            temperature = getNumber(res, 100)
            temperatures.append(temperature)

        power = current * voltage
        if power > 100:
            power = 0

        powers.append(power)

        tempPlot.setData(np.array(temperatures))
        powerPlot.setData(np.array(powers))


def go():
    # Always start by initializing Qt (only once per application)
    app = QtGui.QApplication([])

    # Define a top-level widget to hold everything
    w = QtGui.QWidget()

    # Create some widgets to be placed inside
    btn = QtGui.QPushButton('press me')
    text = QtGui.QLineEdit('enter text')
    listw = QtGui.QListWidget()
    plotTemp = pg.PlotWidget()
    plotPower = pg.PlotWidget()

    # Create a grid layout to manage the widgets size and position
    layout = QtGui.QGridLayout()
    w.setLayout(layout)

    # Add widgets to the layout in their proper positions
    # layout.addWidget(btn, 0, 0)   # button goes in upper-left
    # layout.addWidget(text, 1, 0)   # text edit goes in middle-left
    # layout.addWidget(listw, 2, 0)  # list widget goes in bottom-left
    # plot goes on right side, spanning 3 rows
    layout.addWidget(plotTemp, 0, 0)
    layout.addWidget(plotPower, 1, 0)

    threadPoolExecutor = cf.ThreadPoolExecutor()
    task = threadPoolExecutor.submit(acquire, plotTemp, plotPower)

    # Display the widget as a new window
    w.show()

    sys.exit(app.exec_())


go()
