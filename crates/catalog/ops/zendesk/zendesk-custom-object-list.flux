op zendesk-custom-object-list(include_ui_path: Bool) -> Any
  description "List custom-object definitions, optionally including each definition's UI path"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/custom_objects")
  response = http.request(method: "GET", query: { include_ui_path }, url)
  return response
