op google-gmail-message-send(user_id: String, raw: String) -> Any
  description "Send a Gmail message, supplied as a complete base64url-encoded RFC 2822 message. It is delivered immediately from the token owner's mailbox and cannot be recalled. Needs the `gmail.send` scope. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://gmail.googleapis.com"
  url = fmt("{base}/gmail/v1/users/{user_id}/messages/send")
  content_type = "application/json"
  payload = { raw }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
