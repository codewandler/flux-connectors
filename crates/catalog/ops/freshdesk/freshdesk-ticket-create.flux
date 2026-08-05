op freshdesk-ticket-create(phone: String, name: String, requester_id: Number, subject: String, description: String, status: Number, priority: Number, source: Number, responder_id: Number, type: String, email_config_id: Number, group_id: Number, product_id: Number, tags: List<String>, cc_emails: List<String>, custom_fields: Any) -> Any
  description "Create a ticket. The requester must be identified either by requester_id, or by phone together with name — Freshdesk states this only in prose and its required flags do not capture it"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/tickets")
  content_type = "application/json"
  payload = { cc_emails, custom_fields, description, email_config_id, group_id, name, phone, priority, product_id, requester_id, responder_id, source, status, subject, tags, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
