import numpy as np
import serial
import sys
import concurrent.futures as cf
import time
from collections import deque

from PyQt5 import QtGui
from PyQt5.QtWidgets import QApplication, QMainWindow
import pyqtgraph as pg


def getNumber(raw, factor):
    return float(raw[-5:])/factor


def acquire(plotTemp, plotPower):
    temperatures = deque(maxlen=600)
    powers = deque(maxlen=600)
    times = deque(maxlen=600)

    current = 0
    voltage = 0
    power = 0

    while True:
        try:
            s = serial.Serial('COM5', 115200)

        except:
            continue

        else:
            start = time.time()

            while True:
                try:
                    res = s.readline().decode("utf-8")

                except:
                    continue

                else:
                    res = res[:-1]

                    curTime = time.time() - start
                    temperature = getNumber(res[:5], 100)
                    power = getNumber(res[5:], 100)

                    if power > 100:
                        power = 0

                    times.append(curTime)
                    temperatures.append(temperature)
                    powers.append(power)

                    plotTemp.setData(np.array(times), np.array(temperatures))
                    plotPower.setData(np.array(times), np.array(powers))


def go():
    # Always start by initializing Qt (only once per application)
    app = QtGui.QApplication([])

    # Define a top-level widget to hold everything
    w = QtGui.QWidget()

    # Create some widgets to be placed inside
    btn = QtGui.QPushButton('press me')
    text = QtGui.QLineEdit('enter text')
    listw = QtGui.QListWidget()

    plots = pg.PlotWidget()
    plotsItem = plots.getPlotItem()
    plotsItem.addLegend()
    plotsItem.showGrid(x=True, y=True)

    plotsItem.setLabel("bottom", text="Time", units="s")

    plotTemp = pg.PlotDataItem(
        name="temperature/°C", pen=pg.mkPen('y', width=2))
    plotPower = pg.PlotDataItem(name="power/W", pen=pg.mkPen('r', width=2))

    plots.addItem(plotTemp)
    plots.addItem(plotPower)

    # Create a grid layout to manage the widgets size and position
    layout = QtGui.QGridLayout()
    w.setLayout(layout)

    # Add widgets to the layout in their proper positions
    # layout.addWidget(btn, 0, 0)   # button goes in upper-left
    # layout.addWidget(text, 1, 0)   # text edit goes in middle-left
    # layout.addWidget(listw, 2, 0)  # list widget goes in bottom-left
    # plot goes on right side, spanning 3 rows
    layout.addWidget(plots, 0, 0)

    threadPoolExecutor = cf.ThreadPoolExecutor()
    task = threadPoolExecutor.submit(acquire, plotTemp, plotPower)

    # Display the widget as a new window
    w.show()

    sys.exit(app.exec_())


go()
