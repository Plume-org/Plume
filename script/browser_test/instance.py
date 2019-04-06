#!/usr/bin/python3
from utils import Browser

class InstanceName(Browser):
    def test_name_in_title(self):
        self.get("/")
        self.assertIn("plume-test", self.driver.title)
