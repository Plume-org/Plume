# 🚀 Rocket CSRF Protection Example

🛡️ This is a simple demonstration of how Rocket helps protect your web application against Cross-Site Request Forgery (CSRF) attacks.

## 🏁 Getting Started

1. **Build and Run the Project:** Start the application using Cargo:

   ```bash
   cargo run
   ```

1. **Access the Application:** Your Rocket application should be up and running. Open your web browser and visit `http://127.0.0.1:8000`.

![App Demo](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/lcpfehq4rsjpwrjjehvr.png)

> **Note**<br>
The cookie's expiration coincides with the end of the user's session. This alignment occurs because the token's lifespan is deliberately configured as `None`. By setting the lifespan to `None`, the token implicitly adopts the same lifespan as the session, which is the default behavior. This means that when the user's session ends, the token, being closely tied to it, also expires. This mechanism ensures that the token remains valid and active only as long as the session itself, enhancing security and reliability.

![Session Cookie](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/nbrd2kxsm91kuvtdm240.png)

## 🔒 CSRF Protection

This project incorporates robust CSRF protection mechanisms. CSRF attacks occur when unauthorized requests are made on behalf of a user without their consent. Rocket's CSRF protection ensures that form submissions contain a valid CSRF token, significantly enhancing the security of your web forms. 🌐

## 🧪 Testing CSRF Protection

To rigorously assess the effectiveness of your Cross-Site Request Forgery (CSRF) protection mechanisms, you can employ the following `curl` command:

```bash
curl -X POST -d "authenticity_token=invalid" -d "text=sus" http://127.0.0.1:8000/comments
```

When executing this command, you should anticipate receiving a "403 Forbidden: Unauthorized Access" response from your Rocket application. Concurrently, the application will raise an error message stating "Error: Request lacks X-CSRF-Token." This outcome is a direct consequence of the provided `authenticity_token` being invalid. Furthermore, the request fails to include the essential `X-CSRF-Token` header. Subsequently, the CSRF token validation mechanism within your application rightfully rejects the request.

To further evaluate your CSRF defenses, you can simulate a scenario where the `X-CSRF-Token` header is explicitly set as follows:

```bash
curl -X POST  -H "X-CSRF-Token: invalid" -d "authenticity_token=invalid" -d "text=sus" http://127.0.0.1:8000/comments
```

In this scenario, the Rocket application will respond by displaying the error message "Error: CSRF token verification failed!" This time, both the provided `authenticity_token` and the supplied `X-CSRF-Token` header are determined to be invalid, resulting in a test of your CSRF protection.

## ✅ Successful POST Request

To make a successful POST request, follow these steps:

1. **Obtain a CSRF Token:** In a real application, you would typically obtain a valid CSRF token from a GET request. In our example, the application provides a CSRF token when you access `http://127.0.0.1:8000/comments/new`. You can extract it from the HTML response.

1. **Fill in the Form in Your Browser:** You can fill in the form in your web browser with the CSRF token you obtained. This ensures that your request is processed successfully, and you'll receive a response indicating that the comment has been created. 🎉

![form submit](https://dev-to-uploads.s3.amazonaws.com/uploads/articles/l56i9gg851ggfrnqv5sf.png)

## 📁 Project Structure

- `src/main.rs`: The primary application code.
- `templates/comments/new.html.hbs`: Contains the HTML template for the web page.

This example is designed to provide a straightforward illustration of how Rocket enhances web security through CSRF protection.

Enjoy exploring the project and learning about web security with Rocket! 🚀