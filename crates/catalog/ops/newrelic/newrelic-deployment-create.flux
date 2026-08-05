op newrelic-deployment-create(application_id: String, revision: String, changelog: String, description: String, user: String) -> Any
  description "Record a deployment marker against an application, timestamped now. The marker appears on every chart for that application and is visible to everyone on the account; this connector cannot remove one once recorded. Calling this twice records two deployments"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/applications/{application_id}/deployments.json")
  content_type = "application/json"
  payload = { deployment: { changelog, description, revision, user } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
