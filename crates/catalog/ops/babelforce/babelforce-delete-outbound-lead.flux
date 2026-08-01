op babelforce-delete-outbound-lead(id: String, leadId: String) -> Any
  description "Delete a lead"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads/{leadId}")
  response = http.request(method: "DELETE", url)
  return response
