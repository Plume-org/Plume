#!/usr/bin/python3
from utils import Browser

class NotificationDeletion(Browser):
    def test_regression_651(self):
        self.create_account("user1", "user1@mail", "password")
        self.create_account("user2", "user2@mail", "password")
        
        self.login("user2", "password")
        self.follow("user1")
        self.logout()

        self.login("user1", "password")
        self.get("/notifications")
        self.assertIn("user2", self.driver.get_element_by_xpath("//div[@class='list']"))
        self.logout()
        
        self.login("user2", "password")
        self.delete_account()

        self.login("user1", "password")
        self.assertEqual(200, self.get("/notifications").statusCode())

        self.delete() # cleanup
