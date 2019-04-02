#!/usr/bin/python3
import unittest,os
from selenium import webdriver
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.desired_capabilities import DesiredCapabilities


class Browser(unittest.TestCase):
    def setUp(self):
        if os.environ["BROWSER"] == "firefox":
            self.driver = webdriver.Remote(
                command_executor='http://localhost:24444/wd/hub',
                desired_capabilities=DesiredCapabilities.FIREFOX)
        elif os.environ["BROWSER"] == "chrome":
            self.driver = webdriver.Remote(
                command_executor='http://localhost:24444/wd/hub',
                desired_capabilities=DesiredCapabilities.CHROME)
        else:
            raise Exception("No browser was requested")
    def tearDown(self):
        self.driver.close()


class PythonOrgSearch(Browser):
    def test_search_in_python_org(self):
        driver = self.driver
        driver.get("http://localhost:7878/")
        self.assertIn("plume-test", driver.title)


if __name__ == "__main__":
    unittest.main()
