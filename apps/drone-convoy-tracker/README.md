# ArgoCD ApplicationSets GitOps


## Overall Architecture of ArgoCD ApplicationSets vs Old App-of-Apps




## The Application (Rust Drone Convoy Tracking System)   

The ArgoCD cluster application Helm Chart packages a Rust-Native WebAssembly full-stack application. The server side of the applicaiton includes REST and GraphQL service APIs, WebSockets APIs to update in-realtime attack drone status and the drone's properties to the UI. The server side has a dependency of state tracking data on ScyllaDB NoSQL DB and a write-through cache using Redis clusters.


## The ArgoCD ApplicationSet Architeture

### The ArgoCD Cluster Addons 

### The ArgoCD Cluster Apps


## ArgoCD Feature Promotion w/ Kargo



