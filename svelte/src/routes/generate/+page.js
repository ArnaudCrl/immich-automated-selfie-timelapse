import { redirect } from '@sveltejs/kit';

/** @type {import('./$types').PageLoad} */
export async function load({ fetch }) {
	try {
		const response = await fetch('http://localhost:5000/login', {
			credentials: 'include'
		});
		const data = await response.json();
        console.assert(data.isLoggedIn, 'User is not logged in');
		return {};
	}
	catch (error) {
		throw redirect(302, '/login');
	}
}
