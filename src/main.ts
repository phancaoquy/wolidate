import { mount } from 'svelte'
import App from './App.svelte'
import './styles/tokens.css'
import './styles/layout.css'

const target = document.getElementById('app')
if (!target) throw new Error('#app is missing')

mount(App, { target })
