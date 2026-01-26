import { redirect } from "@sveltejs/kit";

/** @type {import('./$types').PageLoad} */
export async function load({ fetch }) {
  try {
    const response = await fetch("http://localhost:5000/settings", {
      method: "GET",
      credentials: "include",
    });
    const data = await response.json();
    return data;
  } catch (error) {
    throw redirect(302, "/login");
  }
}
