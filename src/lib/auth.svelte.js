class AuthState {
    token = $state(localStorage.getItem('auth_token') || null);
    user = $state(JSON.parse(localStorage.getItem('auth_user') || 'null'));

    get isAuthenticated() {
        return !!this.token;
    }

    login(token, userData) {
        this.token = token;
        this.user = userData;
        localStorage.setItem('auth_token', token);
        localStorage.setItem('auth_user', JSON.stringify(userData));
    }

    logout() {
        this.token = null;
        this.user = null;
        localStorage.removeItem('auth_token');
        localStorage.removeItem('auth_user');
    }
}

export const auth = new AuthState();
