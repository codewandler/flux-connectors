op intercom-conversation-reply(conversation_id: String, message_type: String, admin_id: String, body: String) -> Any
  description "Reply to a conversation as an admin. With message_type `comment` the reply is delivered to the end user by email or in-app message and cannot be un-sent; with `note` it is an internal comment only teammates see. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.intercom.io"
  url = fmt("{base}/conversations/{conversation_id}/reply")
  content_type = "application/json"
  type = "admin"
  payload = { admin_id, body, message_type, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
