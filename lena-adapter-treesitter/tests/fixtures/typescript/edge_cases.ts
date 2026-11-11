class ApiClient {
    async fetchData() {
        console.log("Fetching");
        return await fetch("/api");
    }
}
function standalone() {
    new ApiClient().fetchData();
}
