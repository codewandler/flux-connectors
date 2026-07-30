op zendesk-ticket-tag-add(ticket_id: Number, updated_stamp: String, tags: List<String>) -> Any
  description "Add tags to a ticket without replacing the tags it already has"
  risk "medium"
  idempotency "conditional"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  $content_type = "application/json"
  $safe_update = true
  $payload = { ticket: { additional_tags: $tags, safe_update: $safe_update, updated_stamp: $updated_stamp } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
