import { mount } from 'svelte';
import './styles/global.css';
import App from './App.svelte';

const target = document.getElementById('app');
if (target) {
  mount(App, { target });
}
