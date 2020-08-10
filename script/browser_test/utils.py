#!/usr/bin/python3
import unittest,os
from selenium import webdriver
from selenium.webdriver.common.desired_capabilities import DesiredCapabilities

class Browser(unittest.TestCase):
    def setUp(self):
        if os.environ["BROWSER"] == "firefox":
            capabilities=DesiredCapabilities.FIREFOX
        elif os.environ["BROWSER"] == "chrome":
            capabilities=DesiredCapabilities.CHROME
        else:
            raise Exception("No browser was requested")
        capabilities['acceptSslCerts'] = True
        self.driver = webdriver.Remote(
            # The "selenium" address is set up by Drone CI and "points" to the container running selenium
            command_executor='http://selenium:24444/wd/hub',
            desired_capabilities=capabilities)
    def tearDown(self):
        self.driver.close()

    def get(self, url):
        return self.driver.get("https://localhost" + url)
