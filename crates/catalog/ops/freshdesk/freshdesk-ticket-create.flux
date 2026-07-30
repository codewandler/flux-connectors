op freshdesk-ticket-create(phone: String, name: String, requester_id: Number, subject: String, description: String, status: Number, priority: Number, source: Number, responder_id: Number, type: String, email_config_id: Number, group_id: Number, product_id: Number, tags: List<String>, cc_emails: List<String>, custom_fields: Any) -> Any
  description "Create a ticket. The requester must be identified either by requester_id, or by phone together with name — Freshdesk states this only in prose and its required flags do not capture it"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://{domain}/api/v2"
  $url = fmt("{base}/tickets")
  $content_type = "application/json"
  $payload = { cc_emails: $cc_emails, custom_fields: $custom_fields, description: $description, email_config_id: $email_config_id, group_id: $group_id, name: $name, phone: $phone, priority: $priority, product_id: $product_id, requester_id: $requester_id, responder_id: $responder_id, source: $source, status: $status, subject: $subject, tags: $tags, type: $type }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
