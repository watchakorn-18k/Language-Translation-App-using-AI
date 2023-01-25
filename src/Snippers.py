import sys
import pyperclip
import pyautogui
import easyocr
from PyQt5 import QtCore, QtGui, QtWidgets
from PyQt5.QtCore import Qt


class Snipper(QtWidgets.QWidget):
    def __init__(self, parent, langs_list=None, flags=Qt.WindowFlags()):
        super().__init__(parent=parent, flags=flags)

        self.setWindowTitle("wk-18k")
        self.setWindowFlags(
            Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Dialog
        )
        self.setWindowState(self.windowState() | Qt.WindowFullScreen)
        self._screen = QtWidgets.QApplication.screenAt(QtGui.QCursor.pos())
        palette = QtGui.QPalette()
        palette.setBrush(self.backgroundRole(), QtGui.QBrush(self.getWindow()))
        self.setPalette(palette)
        QtWidgets.QApplication.setOverrideCursor(QtGui.QCursor(QtCore.Qt.CrossCursor))
        self.start, self.end = QtCore.QPoint(), QtCore.QPoint()
        self.langs_list = langs_list

    def getWindow(self):
        return self._screen.grabWindow(0)

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Escape:
            QtWidgets.QApplication.quit()
        return super().keyPressEvent(event)

    def paintEvent(self, event):
        painter = QtGui.QPainter(self)
        painter.setPen(Qt.NoPen)
        painter.setBrush(QtGui.QColor(0, 0, 0, 100))
        painter.drawRect(0, 0, self.width(), self.height())

        if self.start == self.end:
            return super().paintEvent(event)

        painter.setPen(QtGui.QPen(QtGui.QColor(0, 116, 255), 3))
        painter.setBrush(painter.background())
        painter.drawRect(QtCore.QRect(self.start, self.end))
        return super().paintEvent(event)

    def mousePressEvent(self, event):
        self.start = self.end = event.pos()
        self.update()
        return super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        self.end = event.pos()
        self.update()
        return super().mousePressEvent(event)

    def snipOcr(self):
        self.hide()
        return self.ocrOfDrawnRectangle()

    def hide(self):
        super().hide()
        QtWidgets.QApplication.processEvents()

    def ocrOfDrawnRectangle(self):
        self.im = pyautogui.screenshot(
            region=(
                min(self.start.x(), self.end.x()),
                min(self.start.y(), self.end.y()),
                abs(self.start.x() - self.end.x()),
                abs(self.start.y() - self.end.y()),
            )
        )
        self.im.save("orc.png")
        self.reader = easyocr.Reader(self.langs_list, gpu=True)
        self.result = self.reader.readtext("orc.png", detail=0)
        self.string_text = "".join(str(i) for i in self.result)
        pyperclip.copy(self.string_text)
        return self.string_text


class OneTimeSnipper(Snipper):
    """Take an OCR screenshot once then end execution."""

    def mouseReleaseEvent(self, event):
        if self.start == self.end:
            return super().mouseReleaseEvent(event)
        self.snipOcr = self.snipOcr()
        QtWidgets.QApplication.quit()


def main_snipper(langs_list):

    QtCore.QCoreApplication.setAttribute(Qt.AA_DisableHighDpiScaling)
    app = QtWidgets.QApplication(sys.argv)
    window = QtWidgets.QMainWindow()
    snipper = OneTimeSnipper(window, langs_list)
    snipper.show()
    app.exec_()
