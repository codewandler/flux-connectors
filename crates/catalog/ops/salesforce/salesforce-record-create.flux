op salesforce-record-create(sobject_type: String, body: Any) -> Any
  description "Create one SObject record. Which fields are required depends on the SObject type and the org's own validation rules and page layouts — check with salesforce-sobject-describe first"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{instance}.my.salesforce.com"
  url = fmt("{base}/services/data/v59.0/sobjects/{sobject_type}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
