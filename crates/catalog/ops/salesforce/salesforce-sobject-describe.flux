op salesforce-sobject-describe(sobject_type: String) -> Any
  description "Describe an SObject's schema: its label, whether it is custom, its create/update/delete/query permissions, and every field's name, type and constraints. The reference for what salesforce-record-create and salesforce-record-update may send"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{instance}.my.salesforce.com"
  url = fmt("{base}/services/data/v59.0/sobjects/{sobject_type}/describe")
  response = http.request(method: "GET", url)
  return response
