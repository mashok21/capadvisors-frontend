import { mount } from 'svelte'
import './App.css'
import './lib/httpInterceptor.js'
import App from './App.svelte'

const app = mount(App, {
    target: document.getElementById('app'),
})

export default app
