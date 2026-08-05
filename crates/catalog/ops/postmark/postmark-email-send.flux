op postmark-email-send(from: String, to: String, cc: String, bcc: String, subject: String, text_body: String, html_body: String, reply_to: String, tag: String) -> Any
  description "Send a single email immediately from the token's server. Delivered within seconds and cannot be recalled. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/Message`, its error code at `/ErrorCode` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.postmarkapp.com"
  url = fmt("{base}/email")
  content_type = "application/json"
  payload = { Bcc: bcc, Cc: cc, From: from, HtmlBody: html_body, ReplyTo: reply_to, Subject: subject, Tag: tag, TextBody: text_body, To: to }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
