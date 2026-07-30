op zendesk-ticket-update(ticket_id: Number, updated_stamp: String, status: String, priority: String, assignee_id: Number, group_id: Number, type: String) -> Any
  description "Safe-update selected ticket fields against the caller's updated_stamp; at least one of status, priority, assignee_id, group_id or type must be supplied"
  risk "medium"
  idempotency "conditional"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  $content_type = "application/json"
  $safe_update = true
  $payload = { ticket: { assignee_id: $assignee_id, group_id: $group_id, priority: $priority, safe_update: $safe_update, status: $status, type: $type, updated_stamp: $updated_stamp } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
