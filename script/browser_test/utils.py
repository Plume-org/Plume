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
            command_executor='http://localhost:24444/wd/hub',
            desired_capabilities=capabilities)
    def tearDown(self):
        self.driver.close()

    def get(self, url):
        return self.driver.get("https://localhost" + url)

    def create_account(self, name, email, password):
        self.get("/users/new")

        inp = self.driver.find_element_by_id("username")
        inp.send_keys(name)

        inp = self.driver.find_element_by_id("email")
        inp.send_keys(email)

        inp = self.driver.find_element_by_id("password")
        inp.send_keys(password)

        inp = self.driver.find_element_by_id("password_confirmation")
        inp.send_keys(password)


        driver.find_element_by_xpath("//input[@type='submit']").click()

    def delete_account(self):
        self.get("/me")
        self.get("{}/edit".format(self.driver.current_url))

        self.driver.find_element_by_xpath("//input[@type='submit' and contains(@class, 'destructive')]").click()

    def login(self, name, password):
        self.get("/login")

        inp = self.driver.find_element_by_id("eamil_or_name")
        inp.send_keys(name)

        inp = self.driver.find_element_by_id("password")
        inp.send_keys(password)

        driver.find_element_by_xpath("//input[@type='submit']").click()

    def logout(self):
        self.get("/logout")

    def follow(self, other):
        self.get("/@/"+other) 
        driver.find_element_by_xpath("//input[@type='submit']").click()
