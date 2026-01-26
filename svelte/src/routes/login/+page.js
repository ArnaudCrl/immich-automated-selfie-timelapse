/** @type {import('./$types').PageLoad} */
export async function load({ fetch }) {
  try {
    const response = await fetch("http://localhost:5000/login", {
      credentials: "include",
    });
    let data = await response.json();
    return data;
  } catch (error) {
    return {
      IMMICH_BASE_URL: "",
      IMMICH_API_KEY: "",
      isLoggedIn: false,
    };
  }
}
