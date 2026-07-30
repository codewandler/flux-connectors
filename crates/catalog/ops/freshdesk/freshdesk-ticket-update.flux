op freshdesk-ticket-update(id: Number, subject: String, description: String, status: Number, priority: Number, requester_id: Number, responder_id: Number, name: String, phone: String, email: String, type: String, email_config_id: Number, group_id: Number, product_id: Number, tags: List<String>, custom_fields: Any) -> Any
  description "Update a ticket's fields"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/tickets/{id}")
  content_type = "application/json"
  payload = { custom_fields, description, email, email_config_id, group_id, name, phone, priority, product_id, requester_id, responder_id, status, subject, tags, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
